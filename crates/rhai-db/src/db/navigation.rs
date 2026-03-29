use crate::db::DatabaseSnapshot;
use crate::db::imports::{
    dedupe_symbol_locations, imported_global_method_symbols,
    linked_import_targets_for_path_reference, project_identity_for_export,
};
use crate::db::query_support::cached_navigation_target;
use crate::db::rename::project_reference_kind_rank;
use crate::types::{
    LocatedNavigationTarget, LocatedProjectReference, LocatedSymbolIdentity,
    LocatedWorkspaceSymbol, ProjectReferenceKind, ProjectReferences,
};
use rhai_hir::FileBackedSymbolIdentity;
use rhai_syntax::TextSize;
use rhai_vfs::FileId;

impl DatabaseSnapshot {
    pub fn goto_definition(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<LocatedNavigationTarget> {
        self.project_targets_at(file_id, offset)
            .iter()
            .flat_map(|target| self.navigation_targets_for_identity(&target.symbol))
            .collect()
    }

    pub fn find_references(&self, file_id: FileId, offset: TextSize) -> Option<ProjectReferences> {
        let targets = self.project_targets_at(file_id, offset);
        if targets.is_empty() {
            return None;
        }

        Some(ProjectReferences {
            references: self.collect_project_references(&targets),
            targets,
        })
    }

    fn navigation_targets_for_identity(
        &self,
        identity: &FileBackedSymbolIdentity,
    ) -> Vec<LocatedNavigationTarget> {
        let mut targets = self
            .locate_symbol(identity)
            .iter()
            .filter_map(|location| {
                let analysis = self.analysis.get(&location.file_id)?;
                Some(LocatedNavigationTarget {
                    file_id: location.file_id,
                    target: cached_navigation_target(analysis, location.symbol.symbol),
                })
            })
            .collect::<Vec<_>>();

        targets.sort_by(|left, right| {
            left.file_id.0.cmp(&right.file_id.0).then_with(|| {
                left.target
                    .full_range
                    .start()
                    .cmp(&right.target.full_range.start())
            })
        });
        targets
            .dedup_by(|left, right| left.file_id == right.file_id && left.target == right.target);
        targets
    }

    pub(crate) fn project_targets_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<LocatedSymbolIdentity> {
        let Some(analysis) = self.analysis.get(&file_id) else {
            return Vec::new();
        };

        if let Some(reference_id) = analysis.hir.reference_at_offset(offset) {
            let path_targets = linked_import_targets_for_path_reference(
                self,
                file_id,
                analysis.hir.as_ref(),
                reference_id,
            );
            if !path_targets.is_empty() {
                return path_targets;
            }

            if let Some(symbol) = analysis.hir.definition_of(reference_id) {
                return self
                    .locate_symbol(&analysis.hir.file_backed_symbol_identity(symbol))
                    .to_vec();
            }

            if let Some(import_index) = analysis
                .hir
                .imports
                .iter()
                .position(|import| import.module_reference == Some(reference_id))
                && let Some(linked_import) = self.linked_import(file_id, import_index)
            {
                return dedupe_symbol_locations(
                    linked_import
                        .exports
                        .iter()
                        .filter_map(project_identity_for_export)
                        .flat_map(|identity| self.locate_symbol(identity).iter().cloned())
                        .collect(),
                );
            }

            if analysis.hir.reference(reference_id).kind == rhai_hir::ReferenceKind::Field
                && let Some(access) = analysis
                    .hir
                    .member_accesses
                    .iter()
                    .find(|access| access.field_reference == reference_id)
                && let Some(receiver_ty) = analysis
                    .hir
                    .expr_type(access.receiver, &analysis.type_inference.expr_types)
            {
                let imported = imported_global_method_symbols(
                    self,
                    file_id,
                    receiver_ty,
                    analysis.hir.reference(reference_id).name.as_str(),
                );
                if !imported.is_empty() {
                    return imported;
                }
            }

            return Vec::new();
        }

        let Some(symbol) = analysis.hir.symbol_at_offset(offset) else {
            return Vec::new();
        };

        self.locate_symbol(&analysis.hir.file_backed_symbol_identity(symbol))
            .to_vec()
    }

    pub(crate) fn collect_project_references(
        &self,
        targets: &[LocatedSymbolIdentity],
    ) -> Vec<LocatedProjectReference> {
        let mut references = Vec::new();

        for target in targets {
            let Some(analysis) = self.analysis.get(&target.file_id) else {
                continue;
            };

            references.push(LocatedProjectReference {
                file_id: target.file_id,
                range: target.symbol.declaration_range,
                kind: ProjectReferenceKind::Definition,
            });

            references.extend(analysis.hir.references_to(target.symbol.symbol).map(
                |reference_id| LocatedProjectReference {
                    file_id: target.file_id,
                    range: analysis.hir.reference(reference_id).range,
                    kind: ProjectReferenceKind::Reference,
                },
            ));
        }

        references.sort_by(|left, right| {
            left.file_id
                .0
                .cmp(&right.file_id.0)
                .then_with(|| left.range.start().cmp(&right.range.start()))
                .then_with(|| {
                    project_reference_kind_rank(left.kind)
                        .cmp(&project_reference_kind_rank(right.kind))
                })
        });
        references.dedup_by(|left, right| {
            left.file_id == right.file_id && left.range == right.range && left.kind == right.kind
        });
        references
    }
}

pub(crate) fn workspace_symbol_match_rank(
    symbol: &LocatedWorkspaceSymbol,
    query: &str,
) -> (u8, u8, String) {
    let name = symbol.symbol.name.to_ascii_lowercase();
    let container = symbol
        .symbol
        .stable_key
        .container_path
        .join("::")
        .to_ascii_lowercase();

    let name_rank = if query.is_empty() || name == query {
        0
    } else if name.starts_with(query) {
        1
    } else if name.contains(query) {
        2
    } else if container.contains(query) {
        3
    } else {
        4
    };

    let export_rank = if symbol.symbol.exported { 0 } else { 1 };
    (name_rank, export_rank, name)
}
