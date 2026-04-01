use crate::db::DatabaseSnapshot;
use crate::db::imports::{
    imported_global_method_symbols, linked_import_targets_for_path_reference,
};
use crate::db::navigation::outer_scope::{
    resolve_unresolved_name_in_outer_scope, unresolved_outer_scope_references_to_symbol,
};
use crate::db::query_support::cached_navigation_target;
use crate::db::rename::project_reference_kind_rank;
use crate::infer::callable_targets_for_call;
use crate::types::{
    LocatedNavigationTarget, LocatedProjectReference, LocatedSymbolIdentity, ProjectReferenceKind,
    ProjectReferences,
};
use rhai_hir::{FileHir, ReferenceId, SymbolId};
use rhai_syntax::TextSize;
use rhai_vfs::FileId;

pub(crate) mod call_hierarchy;
pub(crate) mod object_fields;
pub(crate) mod outer_scope;
pub(crate) mod type_sources;
pub(crate) mod workspace;

pub(crate) use crate::db::navigation::workspace::workspace_symbol_match_rank;

impl DatabaseSnapshot {
    pub fn goto_definition(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<LocatedNavigationTarget> {
        if let Some(target) = self.goto_import_module_target(file_id, offset) {
            return vec![target];
        }
        if let Some(targets) = self.object_field_goto_targets(file_id, offset)
            && !targets.is_empty()
        {
            return targets;
        }

        self.project_targets_at(file_id, offset)
            .iter()
            .flat_map(|target| self.navigation_targets_for_location(target))
            .collect()
    }

    pub fn goto_type_definition(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<LocatedNavigationTarget> {
        if let Some(targets) = self.object_field_goto_targets(file_id, offset)
            && !targets.is_empty()
        {
            return targets;
        }

        let Some(analysis) = self.analysis.get(&file_id) else {
            return Vec::new();
        };
        let hir = analysis.hir.as_ref();

        if let Some(reference_id) = hir.reference_at_offset(offset)
            && let Some(symbol) = hir
                .definition_of(reference_id)
                .or_else(|| resolve_unresolved_name_in_outer_scope(hir, reference_id))
        {
            let targets = self.type_source_targets_for_symbol(file_id, hir, symbol);
            if !targets.is_empty() {
                return targets;
            }
        }

        if let Some(symbol) = hir.symbol_at_offset(offset) {
            let targets = self.type_source_targets_for_symbol(file_id, hir, symbol);
            if !targets.is_empty() {
                return targets;
            }
        }

        hir.expr_at_offset(offset)
            .map(|expr| self.type_source_targets_for_expr(file_id, hir, expr))
            .unwrap_or_default()
    }

    pub fn find_references(&self, file_id: FileId, offset: TextSize) -> Option<ProjectReferences> {
        if let Some(references) = self.object_field_project_references(file_id, offset) {
            return Some(references);
        }

        let targets = self.project_targets_at(file_id, offset);
        if targets.is_empty() {
            return None;
        }

        Some(ProjectReferences {
            references: self.collect_project_references(&targets),
            targets,
        })
    }

    pub(crate) fn navigation_targets_for_location(
        &self,
        location: &LocatedSymbolIdentity,
    ) -> Vec<LocatedNavigationTarget> {
        let Some(analysis) = self.analysis.get(&location.file_id) else {
            return Vec::new();
        };

        let mut targets = vec![LocatedNavigationTarget {
            file_id: location.file_id,
            target: cached_navigation_target(analysis, location.symbol.symbol),
        }];

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
            let local_overloads = self.local_function_overload_locations_for_reference(
                file_id,
                analysis.hir.as_ref(),
                reference_id,
            );
            if !local_overloads.is_empty() {
                return local_overloads;
            }

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
                return self.symbol_locations_for_file_symbol(
                    file_id,
                    analysis.hir.as_ref(),
                    symbol,
                );
            }
            if let Some(symbol) =
                resolve_unresolved_name_in_outer_scope(analysis.hir.as_ref(), reference_id)
            {
                return self.symbol_locations_for_file_symbol(
                    file_id,
                    analysis.hir.as_ref(),
                    symbol,
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

        self.symbol_locations_for_file_symbol(file_id, analysis.hir.as_ref(), symbol)
    }

    pub(crate) fn local_function_overload_locations_for_reference(
        &self,
        file_id: FileId,
        hir: &FileHir,
        reference_id: ReferenceId,
    ) -> Vec<LocatedSymbolIdentity> {
        let Some(call) = hir
            .calls
            .iter()
            .find(|call| call.callee_reference == Some(reference_id))
        else {
            return Vec::new();
        };
        let Some(analysis) = self.analysis.get(&file_id) else {
            return Vec::new();
        };

        let targets = callable_targets_for_call(
            hir,
            &analysis.type_inference,
            call,
            &self.effective_external_signatures(file_id),
            self.global_functions(),
            self.host_types(),
            &[],
            None,
        );

        let mut locations = targets
            .into_iter()
            .filter_map(|target| target.local_symbol)
            .flat_map(|symbol| self.symbol_locations_for_file_symbol(file_id, hir, symbol))
            .collect::<Vec<_>>();
        locations.sort_by(|left, right| {
            left.file_id
                .0
                .cmp(&right.file_id.0)
                .then_with(|| left.symbol.symbol.0.cmp(&right.symbol.symbol.0))
        });
        locations.dedup();
        locations
    }

    pub(crate) fn symbol_locations_for_file_symbol(
        &self,
        file_id: FileId,
        hir: &FileHir,
        symbol: SymbolId,
    ) -> Vec<LocatedSymbolIdentity> {
        let identity = hir.file_backed_symbol_identity(symbol);
        let locations = self.locate_symbol(&identity);
        if !locations.is_empty() {
            return locations.to_vec();
        }

        vec![LocatedSymbolIdentity {
            file_id,
            symbol: identity,
        }]
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
            references.extend(
                unresolved_outer_scope_references_to_symbol(
                    analysis.hir.as_ref(),
                    target.symbol.symbol,
                )
                .into_iter()
                .map(|reference_id| LocatedProjectReference {
                    file_id: target.file_id,
                    range: analysis.hir.reference(reference_id).range,
                    kind: ProjectReferenceKind::Reference,
                }),
            );

            for (&candidate_file_id, candidate_analysis) in self.analysis.iter() {
                references.extend(
                    candidate_analysis
                        .hir
                        .references
                        .iter()
                        .enumerate()
                        .flat_map(|(index, reference)| {
                            linked_import_targets_for_path_reference(
                                self,
                                candidate_file_id,
                                candidate_analysis.hir.as_ref(),
                                rhai_hir::ReferenceId(index as u32),
                            )
                            .into_iter()
                            .filter(move |resolved| resolved.symbol == target.symbol)
                            .map(move |_| LocatedProjectReference {
                                file_id: candidate_file_id,
                                range: reference.range,
                                kind: ProjectReferenceKind::Reference,
                            })
                        }),
                );
            }
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

    pub(crate) fn goto_import_module_target(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Option<LocatedNavigationTarget> {
        let analysis = self.analysis.get(&file_id)?;
        let import_index = analysis.hir.imports.iter().position(|import| {
            import
                .module_range
                .is_some_and(|module_range| module_range.contains(offset))
        })?;
        let linked_import = self.linked_import(file_id, import_index)?;
        self.file_navigation_target(linked_import.provider_file_id)
    }

    pub(crate) fn file_navigation_target(
        &self,
        file_id: FileId,
    ) -> Option<LocatedNavigationTarget> {
        let analysis = self.analysis.get(&file_id)?;
        let target = analysis
            .document_symbols
            .first()
            .map(|symbol| rhai_hir::NavigationTarget {
                symbol: symbol.symbol,
                kind: symbol.kind,
                full_range: symbol.full_range,
                focus_range: symbol.focus_range,
            })
            .or_else(|| {
                (!analysis.hir.symbols.is_empty())
                    .then(|| cached_navigation_target(analysis, SymbolId(0)))
            })?;

        Some(LocatedNavigationTarget { file_id, target })
    }
}
