use std::collections::HashMap;

use crate::db::{AnalyzerDatabase, DatabaseSnapshot};
use crate::infer::{ImportedMethodSignature, ImportedModuleMember};
use crate::types::{
    ImportedModuleCompletion, LinkedModuleImport, LocatedModuleExport, LocatedSymbolIdentity,
};
use rhai_hir::{FileBackedSymbolIdentity, FileHir, SymbolId, SymbolKind, TypeRef};
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

        if let Some(directives) = self
            .analysis
            .get(&file_id)
            .map(|analysis| analysis.comment_directives.as_ref())
        {
            for import in &importer_hir.imports {
                let Some(alias) = import.alias else {
                    continue;
                };
                let Some(module_name) = import
                    .module_text
                    .as_deref()
                    .and_then(parse_static_import_module_name)
                else {
                    continue;
                };
                if !directives.external_modules.contains(module_name.as_str()) {
                    continue;
                }
                collect_inline_imported_module_members(
                    directives,
                    module_name.as_str(),
                    &[importer_hir.symbol(alias).name.clone()],
                    &mut members,
                );
            }
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
        let mut completions = HashMap::<String, ImportedModuleCompletion>::new();
        let explicit_global_alias = has_import_alias_named(self, file_id, "global");
        let origin_path = resolve_linked_import_origin_path(self, file_id, module_path)
            .unwrap_or_else(|| module_path.to_vec());

        if let Some(linked_import) =
            resolve_linked_import_for_module_path(self, file_id, module_path)
        {
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
                let docs = export_symbol_docs(self, export);
                let Some(name) = export.export.exported_name.clone() else {
                    continue;
                };
                completions.insert(
                    name.clone(),
                    ImportedModuleCompletion {
                        name,
                        kind: identity.kind,
                        origin: Some(origin_path.join("::")),
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
                        origin: Some(
                            nested_linked_import_origin_path(&origin_path, nested).join("::"),
                        ),
                        file_id: None,
                        symbol: None,
                        annotation: None,
                        docs: None,
                    });
            }
        }

        collect_host_module_completions(self, file_id, module_path, &mut completions);
        collect_inline_module_completions(self, file_id, module_path, &mut completions);
        if module_path
            .first()
            .is_some_and(|segment| segment == "global")
            && !explicit_global_alias
        {
            collect_automatic_global_module_completions(
                self,
                file_id,
                module_path,
                &mut completions,
            );
        }

        let mut values = completions.into_values().collect::<Vec<_>>();
        values.sort_by(|left, right| left.name.cmp(&right.name));
        values
    }
}

fn collect_host_module_completions(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    module_path: &[String],
    completions: &mut HashMap<String, ImportedModuleCompletion>,
) {
    let Some(hir) = snapshot.hir(file_id) else {
        return;
    };
    let [alias_name, rest @ ..] = module_path else {
        return;
    };
    if !rest.is_empty() {
        return;
    }

    let Some(import) = hir.imports.iter().find(|import| {
        import
            .alias
            .is_some_and(|alias| hir.symbol(alias).name == *alias_name)
    }) else {
        return;
    };
    let Some(module_name) = import
        .module_text
        .as_deref()
        .and_then(parse_static_import_module_name)
    else {
        return;
    };
    collect_host_module_members(snapshot, module_name.as_str(), None, completions);
}

fn collect_host_module_members(
    snapshot: &DatabaseSnapshot,
    module_name: &str,
    origin_prefix: Option<&str>,
    completions: &mut HashMap<String, ImportedModuleCompletion>,
) {
    let Some(module) = snapshot
        .host_modules()
        .iter()
        .find(|module| module.name == module_name)
    else {
        return;
    };
    let origin = origin_prefix
        .map(|prefix| format!("{prefix}::{module_name}"))
        .unwrap_or_else(|| module_name.to_owned());

    for function in &module.functions {
        completions
            .entry(function.name.clone())
            .or_insert_with(|| ImportedModuleCompletion {
                name: function.name.clone(),
                kind: rhai_hir::SymbolKind::Function,
                origin: Some(origin.clone()),
                file_id: None,
                symbol: None,
                annotation: host_function_annotation(function),
                docs: host_function_docs(function),
            });
    }

    for constant in &module.constants {
        completions
            .entry(constant.name.clone())
            .or_insert_with(|| ImportedModuleCompletion {
                name: constant.name.clone(),
                kind: rhai_hir::SymbolKind::Constant,
                origin: Some(origin.clone()),
                file_id: None,
                symbol: None,
                annotation: constant.ty.clone(),
                docs: constant.docs.clone(),
            });
    }
}

fn collect_automatic_global_module_completions(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    module_path: &[String],
    completions: &mut HashMap<String, ImportedModuleCompletion>,
) {
    let [_global, rest @ ..] = module_path else {
        return;
    };

    if rest.is_empty() {
        collect_file_global_constant_completions(snapshot, file_id, completions);
        collect_global_host_module_roots(snapshot, completions);
        return;
    }

    if rest.len() == 1 {
        collect_host_module_members(snapshot, rest[0].as_str(), Some("global"), completions);
    }
}

fn collect_file_global_constant_completions(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    completions: &mut HashMap<String, ImportedModuleCompletion>,
) {
    let Some(hir) = snapshot.hir(file_id) else {
        return;
    };

    for (index, symbol) in hir.symbols.iter().enumerate() {
        if symbol.kind != rhai_hir::SymbolKind::Constant
            || hir.scope(symbol.scope).kind != rhai_hir::ScopeKind::File
        {
            continue;
        }

        let symbol_id = SymbolId(index as u32);
        completions
            .entry(symbol.name.clone())
            .or_insert_with(|| ImportedModuleCompletion {
                name: symbol.name.clone(),
                kind: rhai_hir::SymbolKind::Constant,
                origin: Some(String::from("global")),
                file_id: Some(file_id),
                symbol: Some(symbol_id),
                annotation: snapshot.inferred_symbol_type(file_id, symbol_id).cloned(),
                docs: symbol.docs.map(|docs| hir.doc_block(docs).text.clone()),
            });
    }
}

fn collect_global_host_module_roots(
    snapshot: &DatabaseSnapshot,
    completions: &mut HashMap<String, ImportedModuleCompletion>,
) {
    for module in snapshot.host_modules() {
        completions
            .entry(module.name.clone())
            .or_insert_with(|| ImportedModuleCompletion {
                name: module.name.clone(),
                kind: rhai_hir::SymbolKind::ImportAlias,
                origin: Some(String::from("global")),
                file_id: None,
                symbol: None,
                annotation: None,
                docs: module.docs.clone(),
            });
    }
}

fn host_function_annotation(function: &crate::HostFunction) -> Option<TypeRef> {
    let mut signatures = function
        .overloads
        .iter()
        .filter_map(|overload| overload.signature.clone());
    let first = signatures.next()?;
    if signatures.next().is_some() {
        None
    } else {
        Some(TypeRef::Function(first))
    }
}

fn host_function_docs(function: &crate::HostFunction) -> Option<String> {
    let mut docs = function
        .overloads
        .iter()
        .filter_map(|overload| overload.docs.as_deref())
        .map(str::trim)
        .filter(|docs| !docs.is_empty())
        .collect::<Vec<_>>();
    docs.sort_unstable();
    docs.dedup();
    (!docs.is_empty()).then(|| docs.join("\n\n"))
}

fn export_symbol_docs(snapshot: &DatabaseSnapshot, export: &LocatedModuleExport) -> Option<String> {
    let hir = snapshot.hir(export.file_id)?;

    export
        .export
        .alias
        .as_ref()
        .and_then(|alias| hir.symbol(alias.symbol).docs)
        .or_else(|| {
            export
                .export
                .target
                .as_ref()
                .and_then(|target| hir.symbol(target.symbol).docs)
        })
        .map(|docs| hir.doc_block(docs).text.clone())
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
    if let Some(path_parts) = rooted_global_path_parts(hir, path_expr) {
        if has_import_alias_named(snapshot, file_id, "global") {
            let mut aliased_parts = vec![String::from("global")];
            aliased_parts.extend(path_parts);
            return resolve_linked_import_path_targets(snapshot, file_id, &aliased_parts);
        }

        return automatic_global_targets_for_path(file_id, hir, &path_parts);
    }
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

fn resolve_linked_import_origin_path(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    module_path: &[String],
) -> Option<Vec<String>> {
    let [alias_name, rest @ ..] = module_path else {
        return None;
    };
    let linked_import = linked_import_for_alias_name(snapshot, file_id, alias_name)?;
    let mut origin = vec![linked_import.module_name.clone()];
    if rest.is_empty() {
        return Some(origin);
    }

    let nested = resolve_linked_import_origin_path(snapshot, linked_import.provider_file_id, rest)?;
    origin.extend(nested);
    Some(origin)
}

fn nested_linked_import_origin_path(
    parent_origin_path: &[String],
    linked_import: &LinkedModuleImport,
) -> Vec<String> {
    let mut origin = parent_origin_path.to_vec();
    origin.push(linked_import.module_name.clone());
    origin
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

fn has_import_alias_named(snapshot: &DatabaseSnapshot, file_id: FileId, alias_name: &str) -> bool {
    let Some(hir) = snapshot.hir(file_id) else {
        return false;
    };

    hir.imports.iter().any(|import| {
        import
            .alias
            .is_some_and(|alias| hir.symbol(alias).name == alias_name)
    })
}

fn rooted_global_path_parts(hir: &FileHir, expr: rhai_hir::ExprId) -> Option<Vec<String>> {
    let path = hir.path_expr(expr)?;
    path.rooted_global
        .then(|| hir.qualified_path_parts(expr))
        .flatten()
}

fn automatic_global_targets_for_path(
    file_id: FileId,
    hir: &FileHir,
    path_parts: &[String],
) -> Vec<LocatedSymbolIdentity> {
    if path_parts.len() != 1 {
        return Vec::new();
    }
    let name = &path_parts[0];

    hir.symbols
        .iter()
        .enumerate()
        .filter_map(|(index, symbol)| {
            (symbol.kind == SymbolKind::Constant
                && hir.scope(symbol.scope).kind == rhai_hir::ScopeKind::File
                && symbol.name == *name)
                .then_some(LocatedSymbolIdentity {
                    file_id,
                    symbol: hir.file_backed_symbol_identity(SymbolId(index as u32)),
                })
        })
        .collect()
}

fn collect_inline_imported_module_members(
    directives: &crate::types::FileCommentDirectives,
    module_name: &str,
    alias_path: &[String],
    members: &mut Vec<ImportedModuleMember>,
) {
    let prefix = format!("{module_name}::");
    for (name, ty) in directives.external_signatures.iter() {
        let Some(rest) = name.strip_prefix(prefix.as_str()) else {
            continue;
        };
        let mut segments = rest.split("::").collect::<Vec<_>>();
        let Some(member_name) = segments.pop() else {
            continue;
        };
        let mut module_path = alias_path.to_vec();
        module_path.extend(segments.into_iter().map(str::to_owned));
        members.push(ImportedModuleMember {
            module_path,
            name: member_name.to_owned(),
            ty: ty.clone(),
        });
    }
}

fn collect_inline_module_completions(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    module_path: &[String],
    completions: &mut HashMap<String, ImportedModuleCompletion>,
) {
    let Some(hir) = snapshot.hir(file_id) else {
        return;
    };
    let Some(directives) = snapshot.comment_directives(file_id) else {
        return;
    };
    let [alias_name, rest @ ..] = module_path else {
        return;
    };

    let Some(import) = hir.imports.iter().find(|import| {
        import
            .alias
            .is_some_and(|alias| hir.symbol(alias).name == *alias_name)
    }) else {
        return;
    };
    let Some(module_name) = import
        .module_text
        .as_deref()
        .and_then(parse_static_import_module_name)
    else {
        return;
    };
    if !directives.external_modules.contains(module_name.as_str()) {
        return;
    }

    let base_prefix = if rest.is_empty() {
        module_name
    } else {
        format!("{module_name}::{}", rest.join("::"))
    };
    let qualified_prefix = format!("{base_prefix}::");
    for (name, ty) in directives.external_signatures.iter() {
        let Some(rest) = name.strip_prefix(qualified_prefix.as_str()) else {
            continue;
        };
        let mut segments = rest.split("::");
        let Some(next_name) = segments.next() else {
            continue;
        };
        let has_more = segments.next().is_some();
        completions
            .entry(next_name.to_owned())
            .or_insert_with(|| ImportedModuleCompletion {
                name: next_name.to_owned(),
                kind: if has_more {
                    rhai_hir::SymbolKind::ImportAlias
                } else if matches!(ty, TypeRef::Function(_)) {
                    rhai_hir::SymbolKind::Function
                } else {
                    rhai_hir::SymbolKind::Constant
                },
                origin: Some(base_prefix.clone()),
                file_id: None,
                symbol: None,
                annotation: (!has_more).then(|| ty.clone()),
                docs: None,
            });
    }
}

fn parse_static_import_module_name(module_text: &str) -> Option<String> {
    if module_text.len() < 2 {
        return None;
    }

    if let Some(text) = module_text
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
        .or_else(|| {
            module_text
                .strip_prefix('`')
                .and_then(|text| text.strip_suffix('`'))
        })
    {
        return Some(text.to_owned());
    }

    if !module_text.starts_with('r') {
        return None;
    }
    let quote = module_text.find('"')?;
    if !module_text.get(1..quote)?.chars().all(|ch| ch == '#') {
        return None;
    }
    let hashes = module_text.get(1..quote)?;
    let suffix = format!("\"{hashes}");
    module_text
        .ends_with(suffix.as_str())
        .then(|| {
            module_text
                .get(quote + 1..module_text.len() - suffix.len())
                .map(str::to_owned)
        })
        .flatten()
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
