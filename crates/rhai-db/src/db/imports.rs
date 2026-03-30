use std::collections::HashMap;

use crate::db::{AnalyzerDatabase, DatabaseSnapshot};
use crate::infer::{ImportedMethodSignature, ImportedModuleMember};
use crate::types::{
    ImportedModuleCompletion, LinkedModuleImport, LocatedModuleExport, LocatedSymbolIdentity,
};
use rhai_hir::{FileBackedSymbolIdentity, FileHir, SymbolId, TypeRef};
use rhai_vfs::FileId;

impl AnalyzerDatabase {
    pub(crate) fn derive_workspace_type_seeds(
        &self,
    ) -> HashMap<FileId, HashMap<SymbolId, TypeRef>> {
        HashMap::new()
    }

    pub(crate) fn imported_method_signatures(
        &self,
        file_id: FileId,
    ) -> Vec<ImportedMethodSignature> {
        self.workspace_indexes
            .linked_imports
            .get(&file_id)
            .into_iter()
            .flat_map(|imports| imports.iter())
            .flat_map(|linked_import| linked_import.exports.iter())
            .filter_map(|export| {
                let identity = project_identity_for_export(export)?;
                (identity.kind == rhai_hir::SymbolKind::Function)
                    .then_some((export.file_id, identity))
            })
            .filter_map(|(provider_file_id, identity)| {
                let provider_hir = self.analysis.get(&provider_file_id)?.hir.clone();
                let this_type = provider_hir
                    .function_info(identity.symbol)?
                    .this_type
                    .clone()?;
                let signature = match provider_hir.symbol(identity.symbol).annotation.as_ref()? {
                    TypeRef::Function(signature) => signature.clone(),
                    _ => return None,
                };
                Some(ImportedMethodSignature {
                    name: identity.name.clone(),
                    receiver: this_type,
                    signature,
                })
            })
            .collect()
    }

    pub(crate) fn imported_module_members(&self, file_id: FileId) -> Vec<ImportedModuleMember> {
        let Some(importer_hir) = self
            .analysis
            .get(&file_id)
            .map(|analysis| analysis.hir.clone())
        else {
            return Vec::new();
        };
        let mut members = Vec::new();
        for linked_import in self
            .workspace_indexes
            .linked_imports
            .get(&file_id)
            .into_iter()
            .flat_map(|imports| imports.iter())
        {
            let Some(alias) = importer_hir.import(linked_import.import).alias else {
                continue;
            };
            let module_path = vec![importer_hir.symbol(alias).name.clone()];
            self.collect_imported_module_members(
                linked_import,
                &module_path,
                &mut Vec::new(),
                &mut members,
            );
        }
        members
    }

    fn collect_imported_module_members(
        &self,
        linked_import: &LinkedModuleImport,
        module_path: &[String],
        visited_files: &mut Vec<FileId>,
        members: &mut Vec<ImportedModuleMember>,
    ) {
        let provider_file_id = linked_import.provider_file_id;
        if visited_files.contains(&provider_file_id) {
            return;
        }
        visited_files.push(provider_file_id);

        for export in linked_import.exports.iter() {
            let Some(exported_name) = export.export.exported_name.as_ref() else {
                continue;
            };
            let Some(identity) = project_identity_for_export(export) else {
                continue;
            };
            let Some(provider_analysis) = self.analysis.get(&export.file_id) else {
                continue;
            };
            let Some(ty) = provider_analysis
                .type_inference
                .symbol_types
                .get(&identity.symbol)
                .cloned()
                .or_else(|| {
                    provider_analysis
                        .hir
                        .declared_symbol_type(identity.symbol)
                        .cloned()
                })
            else {
                continue;
            };
            members.push(ImportedModuleMember {
                module_path: module_path.to_vec(),
                name: exported_name.clone(),
                ty,
            });
        }

        let Some(provider_hir) = self
            .analysis
            .get(&provider_file_id)
            .map(|analysis| analysis.hir.clone())
        else {
            visited_files.pop();
            return;
        };
        for nested in self
            .workspace_indexes
            .linked_imports
            .get(&provider_file_id)
            .into_iter()
            .flat_map(|imports| imports.iter())
        {
            let Some(alias) = provider_hir.import(nested.import).alias else {
                continue;
            };
            let mut nested_path = module_path.to_vec();
            nested_path.push(provider_hir.symbol(alias).name.clone());
            self.collect_imported_module_members(nested, &nested_path, visited_files, members);
        }

        visited_files.pop();
    }
}

impl DatabaseSnapshot {
    pub fn imported_module_completions(
        &self,
        file_id: FileId,
        module_path: &[String],
    ) -> Vec<ImportedModuleCompletion> {
        let Some(linked_import) = resolve_linked_import_for_module_path(self, file_id, module_path)
        else {
            return Vec::new();
        };

        let mut completions = HashMap::<String, ImportedModuleCompletion>::new();

        for export in linked_import.exports.iter() {
            let Some(identity) = project_identity_for_export(export) else {
                continue;
            };
            let annotation = self
                .inferred_symbol_type(export.file_id, identity.symbol)
                .cloned()
                .or_else(|| {
                    self.hir(export.file_id)
                        .and_then(|hir| hir.declared_symbol_type(identity.symbol).cloned())
                });
            let docs = self.hir(export.file_id).and_then(|hir| {
                hir.symbol(identity.symbol)
                    .docs
                    .map(|docs| hir.doc_block(docs).text.clone())
            });
            let Some(name) = export.export.exported_name.clone() else {
                continue;
            };
            completions.insert(
                name.clone(),
                ImportedModuleCompletion {
                    name,
                    kind: identity.kind,
                    file_id: Some(export.file_id),
                    symbol: Some(identity.symbol),
                    annotation,
                    docs,
                },
            );
        }

        let provider_hir = self.hir(linked_import.provider_file_id);
        for nested in self.linked_imports(linked_import.provider_file_id) {
            let Some(alias_symbol) = provider_hir
                .as_ref()
                .and_then(|hir| hir.import(nested.import).alias)
            else {
                continue;
            };
            let name = provider_hir
                .as_ref()
                .map(|hir| hir.symbol(alias_symbol).name.clone())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            completions
                .entry(name.clone())
                .or_insert(ImportedModuleCompletion {
                    name,
                    kind: rhai_hir::SymbolKind::ImportAlias,
                    file_id: None,
                    symbol: None,
                    annotation: None,
                    docs: None,
                });
        }

        let mut values = completions.into_values().collect::<Vec<_>>();
        values.sort_by(|left, right| left.name.cmp(&right.name));
        values
    }
}

pub(crate) fn project_identity_for_export(
    export: &LocatedModuleExport,
) -> Option<&FileBackedSymbolIdentity> {
    export
        .export
        .alias
        .as_ref()
        .or(export.export.target.as_ref())
}

pub(crate) fn linked_import_targets_for_path_reference(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
    reference_id: rhai_hir::ReferenceId,
) -> Vec<LocatedSymbolIdentity> {
    let Some(path_expr) = hir
        .expr_at_offset(hir.reference(reference_id).range.start())
        .filter(|expr| hir.expr(*expr).kind == rhai_hir::ExprKind::Path)
    else {
        return Vec::new();
    };
    let Some(path_parts) = hir.imported_module_path(path_expr).map(|path| path.parts) else {
        return Vec::new();
    };
    resolve_linked_import_path_targets(snapshot, file_id, &path_parts)
}

pub(crate) fn export_matches_identity(
    export: &LocatedModuleExport,
    identity: &FileBackedSymbolIdentity,
) -> bool {
    export
        .export
        .alias
        .as_ref()
        .is_some_and(|alias| alias == identity)
        || (export.export.alias.is_none()
            && export
                .export
                .target
                .as_ref()
                .is_some_and(|target| target == identity))
}

pub(crate) fn dedupe_symbol_locations(
    mut locations: Vec<LocatedSymbolIdentity>,
) -> Vec<LocatedSymbolIdentity> {
    locations.sort_by(|left, right| {
        left.file_id
            .0
            .cmp(&right.file_id.0)
            .then_with(|| {
                left.symbol
                    .declaration_range
                    .start()
                    .cmp(&right.symbol.declaration_range.start())
            })
            .then_with(|| left.symbol.name.cmp(&right.symbol.name))
    });
    locations.dedup_by(|left, right| left.file_id == right.file_id && left.symbol == right.symbol);
    locations
}

fn resolve_linked_import_path_targets(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    path_parts: &[String],
) -> Vec<LocatedSymbolIdentity> {
    resolve_linked_import_path_targets_inner(snapshot, file_id, path_parts, &mut Vec::new())
}

fn resolve_linked_import_for_module_path<'a>(
    snapshot: &'a DatabaseSnapshot,
    file_id: FileId,
    module_path: &[String],
) -> Option<&'a LinkedModuleImport> {
    let [alias_name, rest @ ..] = module_path else {
        return None;
    };
    let linked_import = linked_import_for_alias_name(snapshot, file_id, alias_name)?;
    if rest.is_empty() {
        return Some(linked_import);
    }

    resolve_linked_import_for_module_path(snapshot, linked_import.provider_file_id, rest)
}

fn resolve_linked_import_path_targets_inner(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    path_parts: &[String],
    visited_files: &mut Vec<FileId>,
) -> Vec<LocatedSymbolIdentity> {
    let [alias_name, rest @ ..] = path_parts else {
        return Vec::new();
    };
    if rest.is_empty() || visited_files.contains(&file_id) {
        return Vec::new();
    }
    visited_files.push(file_id);

    let Some(linked_import) = linked_import_for_alias_name(snapshot, file_id, alias_name) else {
        visited_files.pop();
        return Vec::new();
    };

    let result = if rest.len() == 1 {
        let member_name = &rest[0];
        dedupe_symbol_locations(
            linked_import
                .exports
                .iter()
                .filter(|export| {
                    export.export.exported_name.as_deref() == Some(member_name.as_str())
                })
                .filter_map(project_identity_for_export)
                .flat_map(|identity| snapshot.locate_symbol(identity).iter().cloned())
                .collect(),
        )
    } else {
        let provider_file_id = linked_import.provider_file_id;
        resolve_linked_import_path_targets_inner(snapshot, provider_file_id, rest, visited_files)
    };

    visited_files.pop();
    result
}

fn linked_import_for_alias_name<'a>(
    snapshot: &'a DatabaseSnapshot,
    file_id: FileId,
    alias_name: &str,
) -> Option<&'a LinkedModuleImport> {
    let hir = snapshot.hir(file_id)?;
    snapshot
        .linked_imports(file_id)
        .iter()
        .find(|linked_import| {
            hir.import(linked_import.import)
                .alias
                .is_some_and(|alias| hir.symbol(alias).name == alias_name)
        })
}

pub(crate) fn imported_global_method_symbols(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    receiver_ty: &TypeRef,
    method_name: &str,
) -> Vec<LocatedSymbolIdentity> {
    let mut matches = snapshot
        .linked_imports(file_id)
        .iter()
        .flat_map(|linked_import| linked_import.exports.iter())
        .filter_map(|export| {
            let identity = project_identity_for_export(export)?;
            (identity.kind == rhai_hir::SymbolKind::Function && identity.name == method_name)
                .then_some((export.file_id, identity))
        })
        .filter_map(|(provider_file_id, identity)| {
            let provider_hir = snapshot.hir(provider_file_id)?;
            let this_type = provider_hir
                .function_info(identity.symbol)?
                .this_type
                .as_ref()?;
            receiver_matches_method_type(receiver_ty, this_type)
                .then_some(snapshot.locate_symbol(identity).iter().cloned())
        })
        .flatten()
        .collect::<Vec<_>>();

    matches.sort_by(|left, right| {
        left.file_id
            .0
            .cmp(&right.file_id.0)
            .then_with(|| {
                left.symbol
                    .declaration_range
                    .start()
                    .cmp(&right.symbol.declaration_range.start())
            })
            .then_with(|| left.symbol.name.cmp(&right.symbol.name))
    });
    matches.dedup_by(|left, right| left.file_id == right.file_id && left.symbol == right.symbol);
    matches
}

pub(crate) fn receiver_matches_method_type(receiver: &TypeRef, expected: &TypeRef) -> bool {
    if receiver == expected {
        return true;
    }

    match (receiver, expected) {
        (TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic | TypeRef::Never, _) => true,
        (TypeRef::Union(items), expected) | (TypeRef::Ambiguous(items), expected) => items
            .iter()
            .any(|item| receiver_matches_method_type(item, expected)),
        (TypeRef::Nullable(inner), expected) => receiver_matches_method_type(inner, expected),
        (TypeRef::Applied { name, .. }, TypeRef::Named(expected_name))
        | (
            TypeRef::Named(name),
            TypeRef::Applied {
                name: expected_name,
                ..
            },
        ) => name == expected_name,
        (
            TypeRef::Applied { name, args },
            TypeRef::Applied {
                name: expected_name,
                args: expected_args,
            },
        ) => {
            name == expected_name
                && args.len() == expected_args.len()
                && args
                    .iter()
                    .zip(expected_args.iter())
                    .all(|(arg, expected)| receiver_matches_method_type(arg, expected))
        }
        _ => false,
    }
}
