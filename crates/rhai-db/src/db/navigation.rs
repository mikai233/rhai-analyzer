use crate::db::DatabaseSnapshot;
use crate::db::imports::{
    imported_global_method_symbols, linked_import_targets_for_path_reference,
};
use crate::db::query_support::cached_navigation_target;
use crate::db::rename::project_reference_kind_rank;
use crate::infer::{
    callable_targets_for_call, field_value_exprs_from_expr, field_value_exprs_from_symbol,
    largest_inner_expr,
};
use crate::types::{
    LocatedCallHierarchyItem, LocatedIncomingCall, LocatedNavigationTarget, LocatedOutgoingCall,
    LocatedProjectReference, LocatedSymbolIdentity, LocatedWorkspaceSymbol, ObjectFieldHoverInfo,
    ProjectReferenceKind, ProjectReferences,
};
use rhai_hir::TypeRef;
use rhai_hir::{FileBackedSymbolIdentity, FileHir, ReferenceId, ScopeId, SymbolId, SymbolKind};
use rhai_syntax::{TextRange, TextSize};
use rhai_vfs::FileId;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ObjectFieldDeclKey {
    file_id: FileId,
    start: TextSize,
    end: TextSize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObjectFieldDecl {
    key: ObjectFieldDeclKey,
    name: String,
    owner_symbol: Option<SymbolId>,
    owner_kind: Option<SymbolKind>,
    value_expr: Option<rhai_hir::ExprId>,
}

impl ObjectFieldDecl {
    fn range(&self) -> TextRange {
        TextRange::new(self.key.start, self.key.end)
    }
}

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

    pub fn object_field_hover(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Option<ObjectFieldHoverInfo> {
        let declarations = self.object_field_declarations_at(file_id, offset)?;
        if declarations.is_empty() {
            return None;
        }

        let mut declared_annotation = None::<TypeRef>;
        let mut inferred_annotation = None::<TypeRef>;
        let mut docs = Vec::new();

        for declaration in &declarations {
            let Some(provider_analysis) = self.analysis.get(&declaration.key.file_id) else {
                continue;
            };
            let provider_hir = provider_analysis.hir.as_ref();

            if let Some(symbol) = declaration.owner_symbol
                && let Some(field) = provider_hir
                    .documented_fields(symbol)
                    .into_iter()
                    .find(|field| field.name == declaration.name)
            {
                declared_annotation = Some(match declared_annotation {
                    Some(ref current) => join_hover_types(current, &field.annotation),
                    None => field.annotation,
                });
                if let Some(field_docs) = field.docs.filter(|docs| !docs.trim().is_empty()) {
                    docs.push(field_docs);
                }
            }

            if let Some(value_expr) = declaration.value_expr
                && let Some(value_ty) = provider_analysis
                    .hir
                    .expr_type(value_expr, &provider_analysis.type_inference.expr_types)
            {
                inferred_annotation = Some(match inferred_annotation {
                    Some(ref current) => join_hover_types(current, value_ty),
                    None => value_ty.clone(),
                });
            }
        }

        if inferred_annotation.is_none()
            && let Some(usage_ty) = self.inferred_expr_type_at(file_id, offset)
        {
            inferred_annotation = Some(usage_ty.clone());
        }

        let declaration = declarations.first()?;
        docs.sort_unstable();
        docs.dedup();
        Some(ObjectFieldHoverInfo {
            name: declaration.name.clone(),
            declaration_range: declaration.range(),
            declared_annotation,
            inferred_annotation,
            docs: (!docs.is_empty()).then(|| docs.join("\n\n")),
        })
    }

    fn navigation_targets_for_location(
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

    fn local_function_overload_locations_for_reference(
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

    fn symbol_locations_for_file_symbol(
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

    fn object_field_goto_targets(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Option<Vec<LocatedNavigationTarget>> {
        let declarations = self.object_field_declarations_at(file_id, offset)?;
        if declarations.is_empty() {
            return None;
        }

        let mut targets = declarations
            .into_iter()
            .map(|declaration| LocatedNavigationTarget {
                file_id: declaration.key.file_id,
                target: rhai_hir::NavigationTarget {
                    symbol: declaration.owner_symbol.unwrap_or(SymbolId(0)),
                    kind: declaration.owner_kind.unwrap_or(SymbolKind::Variable),
                    full_range: declaration.range(),
                    focus_range: declaration.range(),
                },
            })
            .collect::<Vec<_>>();

        targets.sort_by(|left, right| {
            left.file_id
                .0
                .cmp(&right.file_id.0)
                .then_with(|| {
                    left.target
                        .full_range
                        .start()
                        .cmp(&right.target.full_range.start())
                })
                .then_with(|| {
                    left.target
                        .full_range
                        .end()
                        .cmp(&right.target.full_range.end())
                })
        });
        targets.dedup_by(|left, right| {
            left.file_id == right.file_id && left.target.full_range == right.target.full_range
        });
        Some(targets)
    }

    fn object_field_project_references(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Option<ProjectReferences> {
        let declarations = self.object_field_declarations_at(file_id, offset)?;
        if declarations.is_empty() {
            return None;
        }

        let mut references = declarations
            .iter()
            .map(|declaration| LocatedProjectReference {
                file_id: declaration.key.file_id,
                range: declaration.range(),
                kind: ProjectReferenceKind::Definition,
            })
            .collect::<Vec<_>>();
        let declaration_keys = declarations.iter().map(|decl| decl.key).collect::<Vec<_>>();

        for (&candidate_file_id, candidate_analysis) in self.analysis.iter() {
            for (index, reference) in candidate_analysis.hir.references.iter().enumerate() {
                if reference.kind != rhai_hir::ReferenceKind::Field {
                    continue;
                }
                let reference_id = ReferenceId(index as u32);
                if self.object_field_reference_matches_declarations(
                    candidate_file_id,
                    candidate_analysis.hir.as_ref(),
                    reference_id,
                    &declaration_keys,
                ) {
                    references.push(LocatedProjectReference {
                        file_id: candidate_file_id,
                        range: reference.range,
                        kind: ProjectReferenceKind::Reference,
                    });
                }
            }
        }

        references.sort_by(|left, right| {
            left.file_id
                .0
                .cmp(&right.file_id.0)
                .then_with(|| left.range.start().cmp(&right.range.start()))
                .then_with(|| left.range.end().cmp(&right.range.end()))
                .then_with(|| {
                    project_reference_kind_rank(left.kind)
                        .cmp(&project_reference_kind_rank(right.kind))
                })
        });
        references.dedup_by(|left, right| {
            left.file_id == right.file_id && left.range == right.range && left.kind == right.kind
        });

        let mut targets = Vec::new();
        for declaration in declarations {
            let Some(symbol) = declaration.owner_symbol else {
                continue;
            };
            let Some(analysis) = self.analysis.get(&declaration.key.file_id) else {
                continue;
            };
            targets.extend(self.symbol_locations_for_file_symbol(
                declaration.key.file_id,
                analysis.hir.as_ref(),
                symbol,
            ));
        }
        targets.sort_by(|left, right| {
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
        targets
            .dedup_by(|left, right| left.file_id == right.file_id && left.symbol == right.symbol);

        Some(ProjectReferences {
            targets,
            references,
        })
    }

    fn object_field_reference_matches_declarations(
        &self,
        file_id: FileId,
        hir: &FileHir,
        reference_id: ReferenceId,
        declaration_keys: &[ObjectFieldDeclKey],
    ) -> bool {
        self.object_field_declarations_for_reference(file_id, hir, reference_id)
            .iter()
            .any(|declaration| declaration_keys.contains(&declaration.key))
    }

    fn object_field_declarations_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Option<Vec<ObjectFieldDecl>> {
        let analysis = self.analysis.get(&file_id)?;
        let hir = analysis.hir.as_ref();

        if let Some(reference_id) = hir.reference_at_offset(offset)
            && hir.reference(reference_id).kind == rhai_hir::ReferenceKind::Field
        {
            let declarations =
                self.object_field_declarations_for_reference(file_id, hir, reference_id);
            if !declarations.is_empty() {
                return Some(declarations);
            }
        }

        let field = hir
            .object_fields
            .iter()
            .find(|field| field.range.contains(offset))?;
        let key = ObjectFieldDeclKey {
            file_id,
            start: field.range.start(),
            end: field.range.end(),
        };
        let owner_symbol = self.owner_symbol_for_object_field(file_id, hir, field.range);
        Some(vec![ObjectFieldDecl {
            key,
            name: field.name.clone(),
            owner_symbol,
            owner_kind: owner_symbol.map(|symbol| hir.symbol(symbol).kind),
            value_expr: field.value,
        }])
    }

    fn object_field_declarations_for_reference(
        &self,
        file_id: FileId,
        hir: &FileHir,
        reference_id: ReferenceId,
    ) -> Vec<ObjectFieldDecl> {
        let Some(access) = hir
            .member_accesses
            .iter()
            .find(|access| access.field_reference == reference_id)
        else {
            return Vec::new();
        };
        let field_name = hir.reference(reference_id).name.as_str();
        self.object_field_declarations_for_receiver(file_id, hir, access.receiver, field_name)
    }

    fn object_field_declarations_for_receiver(
        &self,
        file_id: FileId,
        hir: &FileHir,
        receiver_expr: rhai_hir::ExprId,
        field_name: &str,
    ) -> Vec<ObjectFieldDecl> {
        let mut declarations = self.object_field_decls_from_value_exprs(
            file_id,
            hir,
            field_name,
            field_value_exprs_from_expr(hir, receiver_expr, field_name),
            None,
        );

        if hir.expr(receiver_expr).kind == rhai_hir::ExprKind::Path {
            declarations.extend(self.object_field_decls_from_path_receiver(
                file_id,
                hir,
                receiver_expr,
                field_name,
            ));
        }

        let mut by_key = BTreeMap::<ObjectFieldDeclKey, ObjectFieldDecl>::new();
        for declaration in declarations {
            by_key.entry(declaration.key).or_insert(declaration);
        }
        by_key.into_values().collect()
    }

    fn object_field_decls_from_path_receiver(
        &self,
        file_id: FileId,
        hir: &FileHir,
        receiver_expr: rhai_hir::ExprId,
        field_name: &str,
    ) -> Vec<ObjectFieldDecl> {
        let Some(path) = hir.path_expr(receiver_expr) else {
            return Vec::new();
        };
        let Some(reference_id) = path.segments.last().copied() else {
            return Vec::new();
        };

        linked_import_targets_for_path_reference(self, file_id, hir, reference_id)
            .into_iter()
            .flat_map(|target| {
                let provider = self.analysis.get(&target.file_id)?;
                Some(self.object_field_decls_from_symbol(
                    target.file_id,
                    provider.hir.as_ref(),
                    target.symbol.symbol,
                    field_name,
                ))
            })
            .flatten()
            .collect()
    }

    fn object_field_decls_from_symbol(
        &self,
        file_id: FileId,
        hir: &FileHir,
        symbol: SymbolId,
        field_name: &str,
    ) -> Vec<ObjectFieldDecl> {
        self.object_field_decls_from_value_exprs(
            file_id,
            hir,
            field_name,
            field_value_exprs_from_symbol(hir, symbol, field_name),
            Some(symbol),
        )
    }

    fn object_field_decls_from_value_exprs(
        &self,
        file_id: FileId,
        hir: &FileHir,
        field_name: &str,
        value_exprs: Vec<rhai_hir::ExprId>,
        owner_symbol_hint: Option<SymbolId>,
    ) -> Vec<ObjectFieldDecl> {
        let mut declarations = Vec::new();
        for value_expr in value_exprs {
            for field in hir
                .object_fields
                .iter()
                .filter(|field| field.name == field_name && field.value == Some(value_expr))
            {
                let key = ObjectFieldDeclKey {
                    file_id,
                    start: field.range.start(),
                    end: field.range.end(),
                };
                let owner_symbol = owner_symbol_hint
                    .or_else(|| self.owner_symbol_for_object_field(file_id, hir, field.range));
                declarations.push(ObjectFieldDecl {
                    key,
                    name: field.name.clone(),
                    owner_symbol,
                    owner_kind: owner_symbol.map(|symbol| hir.symbol(symbol).kind),
                    value_expr: field.value,
                });
            }
        }
        declarations
    }

    fn owner_symbol_for_object_field(
        &self,
        _file_id: FileId,
        hir: &FileHir,
        range: TextRange,
    ) -> Option<SymbolId> {
        for (index, _) in hir.symbols.iter().enumerate() {
            let symbol = SymbolId(index as u32);
            if hir
                .value_flows_into(symbol)
                .any(|flow| hir.expr(flow.expr).range.contains_range(range))
            {
                return Some(symbol);
            }
            if hir
                .symbol_mutations_into(symbol)
                .any(|mutation| hir.expr(mutation.value).range.contains_range(range))
            {
                return Some(symbol);
            }
        }
        None
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

    pub(crate) fn call_hierarchy_items_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<LocatedCallHierarchyItem> {
        let mut items = self
            .project_targets_at(file_id, offset)
            .into_iter()
            .filter(|target| target.symbol.kind == rhai_hir::SymbolKind::Function)
            .filter_map(|target| self.call_hierarchy_item_from_identity(&target.symbol))
            .collect::<Vec<_>>();

        items.sort_by(|left, right| {
            left.file_id
                .0
                .cmp(&right.file_id.0)
                .then_with(|| {
                    left.target
                        .full_range
                        .start()
                        .cmp(&right.target.full_range.start())
                })
                .then_with(|| left.symbol.name.cmp(&right.symbol.name))
        });
        items.dedup_by(|left, right| left.file_id == right.file_id && left.symbol == right.symbol);
        items
    }

    pub(crate) fn call_hierarchy_incoming_calls(
        &self,
        item: &FileBackedSymbolIdentity,
    ) -> Vec<LocatedIncomingCall> {
        let mut grouped = BTreeMap::<
            (FileId, u32, u32, String),
            (LocatedCallHierarchyItem, Vec<rhai_syntax::TextRange>),
        >::new();

        for (&file_id, analysis) in self.analysis.iter() {
            for call in &analysis.hir.calls {
                let Some(caller) = analysis
                    .hir
                    .enclosing_function_symbol_at(call.range.start())
                    .filter(|caller| {
                        analysis.hir.symbol(*caller).kind == rhai_hir::SymbolKind::Function
                    })
                else {
                    continue;
                };

                if !call_targets_symbol(self, file_id, analysis.hir.as_ref(), call, item) {
                    continue;
                }

                let caller_identity = analysis.hir.file_backed_symbol_identity(caller);
                let Some(caller_item) = self.call_hierarchy_item_from_identity(&caller_identity)
                else {
                    continue;
                };
                let key = (
                    caller_item.file_id,
                    u32::from(caller_item.target.full_range.start()),
                    u32::from(caller_item.target.full_range.end()),
                    caller_item.symbol.name.clone(),
                );
                grouped
                    .entry(key)
                    .or_insert_with(|| (caller_item, Vec::new()))
                    .1
                    .push(call.callee_range.unwrap_or(call.range));
            }
        }

        grouped
            .into_values()
            .map(|(from, mut from_ranges)| {
                from_ranges.sort_by_key(|range| (range.start(), range.end()));
                from_ranges.dedup();
                LocatedIncomingCall {
                    from,
                    from_ranges: Arc::<[rhai_syntax::TextRange]>::from(from_ranges),
                }
            })
            .collect()
    }

    pub(crate) fn call_hierarchy_outgoing_calls(
        &self,
        item: &FileBackedSymbolIdentity,
    ) -> Vec<LocatedOutgoingCall> {
        let mut grouped = BTreeMap::<
            (FileId, u32, u32, String),
            (LocatedCallHierarchyItem, Vec<rhai_syntax::TextRange>),
        >::new();

        for location in self.locate_symbol(item) {
            let Some(analysis) = self.analysis.get(&location.file_id) else {
                continue;
            };
            let symbol = location.symbol.symbol;
            if analysis.hir.symbol(symbol).kind != rhai_hir::SymbolKind::Function {
                continue;
            }

            for call in analysis.hir.calls.iter().filter(|call| {
                analysis
                    .hir
                    .enclosing_function_symbol_at(call.range.start())
                    == Some(symbol)
            }) {
                for target in call_target_items(self, location.file_id, analysis.hir.as_ref(), call)
                {
                    if target.symbol == *item {
                        continue;
                    }
                    let key = (
                        target.file_id,
                        u32::from(target.target.full_range.start()),
                        u32::from(target.target.full_range.end()),
                        target.symbol.name.clone(),
                    );
                    grouped
                        .entry(key)
                        .or_insert_with(|| (target, Vec::new()))
                        .1
                        .push(call.callee_range.unwrap_or(call.range));
                }
            }
        }

        grouped
            .into_values()
            .map(|(to, mut from_ranges)| {
                from_ranges.sort_by_key(|range| (range.start(), range.end()));
                from_ranges.dedup();
                LocatedOutgoingCall {
                    to,
                    from_ranges: Arc::<[rhai_syntax::TextRange]>::from(from_ranges),
                }
            })
            .collect()
    }

    fn call_hierarchy_item_from_identity(
        &self,
        identity: &FileBackedSymbolIdentity,
    ) -> Option<LocatedCallHierarchyItem> {
        let location = self.locate_symbol(identity).first()?.clone();
        let analysis = self.analysis.get(&location.file_id)?;

        Some(LocatedCallHierarchyItem {
            file_id: location.file_id,
            symbol: location.symbol.clone(),
            target: cached_navigation_target(analysis, location.symbol.symbol),
        })
    }

    fn goto_import_module_target(
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

    fn file_navigation_target(&self, file_id: FileId) -> Option<LocatedNavigationTarget> {
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

    fn type_source_targets_for_symbol(
        &self,
        file_id: FileId,
        hir: &FileHir,
        symbol: SymbolId,
    ) -> Vec<LocatedNavigationTarget> {
        let exprs = type_source_exprs_from_symbol(hir, symbol);
        let mut targets =
            self.navigation_targets_for_type_source_exprs(file_id, hir, exprs, Some(symbol));
        if targets.is_empty()
            && let Some(target) = documented_type_target(file_id, hir, symbol)
        {
            targets.push(target);
        }
        targets
    }

    fn type_source_targets_for_expr(
        &self,
        file_id: FileId,
        hir: &FileHir,
        expr: rhai_hir::ExprId,
    ) -> Vec<LocatedNavigationTarget> {
        let exprs = type_source_exprs_from_expr(hir, expr);
        self.navigation_targets_for_type_source_exprs(file_id, hir, exprs, None)
    }

    fn navigation_targets_for_type_source_exprs(
        &self,
        file_id: FileId,
        hir: &FileHir,
        exprs: Vec<rhai_hir::ExprId>,
        owner_symbol_hint: Option<SymbolId>,
    ) -> Vec<LocatedNavigationTarget> {
        let mut targets = exprs
            .into_iter()
            .map(|expr| {
                let range = hir.expr(expr).range;
                let owner_symbol = owner_symbol_hint.or_else(|| owner_symbol_for_expr(hir, expr));
                LocatedNavigationTarget {
                    file_id,
                    target: rhai_hir::NavigationTarget {
                        symbol: owner_symbol.unwrap_or(SymbolId(0)),
                        kind: owner_symbol
                            .map(|symbol| hir.symbol(symbol).kind)
                            .unwrap_or(SymbolKind::Variable),
                        full_range: range,
                        focus_range: range,
                    },
                }
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
}

fn type_source_exprs_from_expr(hir: &FileHir, expr: rhai_hir::ExprId) -> Vec<rhai_hir::ExprId> {
    let mut visited_exprs = BTreeSet::<u32>::new();
    let mut visited_symbols = BTreeSet::<u32>::new();
    let mut exprs = Vec::<rhai_hir::ExprId>::new();
    collect_type_source_exprs_from_expr(
        hir,
        expr,
        &mut visited_exprs,
        &mut visited_symbols,
        &mut exprs,
    );
    exprs.sort_by_key(|expr| expr.0);
    exprs.dedup_by_key(|expr| expr.0);
    exprs
}

fn type_source_exprs_from_symbol(hir: &FileHir, symbol: SymbolId) -> Vec<rhai_hir::ExprId> {
    let mut visited_exprs = BTreeSet::<u32>::new();
    let mut visited_symbols = BTreeSet::<u32>::new();
    let mut exprs = Vec::<rhai_hir::ExprId>::new();
    collect_type_source_exprs_from_symbol(
        hir,
        symbol,
        &mut visited_exprs,
        &mut visited_symbols,
        &mut exprs,
    );
    exprs.sort_by_key(|expr| expr.0);
    exprs.dedup_by_key(|expr| expr.0);
    exprs
}

fn collect_type_source_exprs_from_expr(
    hir: &FileHir,
    expr: rhai_hir::ExprId,
    visited_exprs: &mut BTreeSet<u32>,
    visited_symbols: &mut BTreeSet<u32>,
    out: &mut Vec<rhai_hir::ExprId>,
) {
    if !visited_exprs.insert(expr.0) {
        return;
    }

    match hir.expr(expr).kind {
        rhai_hir::ExprKind::Object | rhai_hir::ExprKind::Array | rhai_hir::ExprKind::Closure => {
            out.push(expr);
        }
        rhai_hir::ExprKind::Paren => {
            if let Some(inner) = largest_inner_expr(hir, expr) {
                collect_type_source_exprs_from_expr(
                    hir,
                    inner,
                    visited_exprs,
                    visited_symbols,
                    out,
                );
            }
        }
        rhai_hir::ExprKind::Name => {
            if let Some(symbol) = symbol_for_expr(hir, expr) {
                collect_type_source_exprs_from_symbol(
                    hir,
                    symbol,
                    visited_exprs,
                    visited_symbols,
                    out,
                );
            }
        }
        rhai_hir::ExprKind::Field => {
            if let Some(access) = hir.member_access(expr) {
                let field_name = hir.reference(access.field_reference).name.as_str();
                for value_expr in field_value_exprs_from_expr(hir, access.receiver, field_name) {
                    collect_type_source_exprs_from_expr(
                        hir,
                        value_expr,
                        visited_exprs,
                        visited_symbols,
                        out,
                    );
                }
            }
        }
        rhai_hir::ExprKind::Block => {
            if let Some(block) = hir.block_expr(expr)
                && let Some(tail) = hir.body_tail_value(block.body)
            {
                collect_type_source_exprs_from_expr(hir, tail, visited_exprs, visited_symbols, out);
            }
        }
        rhai_hir::ExprKind::If => {
            if let Some(if_expr) = hir.if_expr(expr) {
                for branch in [if_expr.then_branch, if_expr.else_branch]
                    .into_iter()
                    .flatten()
                {
                    collect_type_source_exprs_from_expr(
                        hir,
                        branch,
                        visited_exprs,
                        visited_symbols,
                        out,
                    );
                }
            }
        }
        rhai_hir::ExprKind::Switch => {
            if let Some(switch_expr) = hir.switch_expr(expr) {
                for arm in switch_expr.arms.iter().flatten().copied() {
                    collect_type_source_exprs_from_expr(
                        hir,
                        arm,
                        visited_exprs,
                        visited_symbols,
                        out,
                    );
                }
            }
        }
        _ => {}
    }
}

fn collect_type_source_exprs_from_symbol(
    hir: &FileHir,
    symbol: SymbolId,
    visited_exprs: &mut BTreeSet<u32>,
    visited_symbols: &mut BTreeSet<u32>,
    out: &mut Vec<rhai_hir::ExprId>,
) {
    if !visited_symbols.insert(symbol.0) {
        return;
    }

    for flow in hir.value_flows_into(symbol) {
        collect_type_source_exprs_from_expr(hir, flow.expr, visited_exprs, visited_symbols, out);
    }

    for mutation in hir.symbol_mutations_into(symbol) {
        match &mutation.kind {
            rhai_hir::SymbolMutationKind::Path { segments } if segments.is_empty() => {
                collect_type_source_exprs_from_expr(
                    hir,
                    mutation.value,
                    visited_exprs,
                    visited_symbols,
                    out,
                );
            }
            _ => {}
        }
    }
}

fn symbol_for_expr(hir: &FileHir, expr: rhai_hir::ExprId) -> Option<SymbolId> {
    (hir.expr(expr).kind == rhai_hir::ExprKind::Name)
        .then(|| hir.reference_at(hir.expr(expr).range))
        .flatten()
        .and_then(|reference| hir.definition_of(reference))
}

fn owner_symbol_for_expr(hir: &FileHir, expr: rhai_hir::ExprId) -> Option<SymbolId> {
    for (index, _) in hir.symbols.iter().enumerate() {
        let symbol = SymbolId(index as u32);
        if hir.value_flows_into(symbol).any(|flow| flow.expr == expr) {
            return Some(symbol);
        }
        if hir
            .symbol_mutations_into(symbol)
            .any(|mutation| mutation.value == expr)
        {
            return Some(symbol);
        }
    }
    None
}

fn documented_type_target(
    file_id: FileId,
    hir: &FileHir,
    symbol: SymbolId,
) -> Option<LocatedNavigationTarget> {
    let symbol_data = hir.symbol(symbol);
    symbol_data.annotation.as_ref()?;
    let doc_id = symbol_data.docs?;
    let docs = hir.doc_block(doc_id);
    Some(LocatedNavigationTarget {
        file_id,
        target: rhai_hir::NavigationTarget {
            symbol,
            kind: symbol_data.kind,
            full_range: docs.range,
            focus_range: docs.range,
        },
    })
}

fn join_hover_types(left: &TypeRef, right: &TypeRef) -> TypeRef {
    if left == right {
        return left.clone();
    }

    match (left, right) {
        (TypeRef::Unknown | TypeRef::Never, other) => other.clone(),
        (other, TypeRef::Unknown | TypeRef::Never) => other.clone(),
        (TypeRef::Object(left_fields), TypeRef::Object(right_fields)) => {
            let mut merged = left_fields.clone();
            for (name, right_ty) in right_fields {
                let next = match merged.get(name.as_str()) {
                    Some(left_ty) => join_hover_types(left_ty, right_ty),
                    None => right_ty.clone(),
                };
                merged.insert(name.clone(), next);
            }
            TypeRef::Object(merged)
        }
        (TypeRef::Array(left_inner), TypeRef::Array(right_inner)) => {
            TypeRef::Array(Box::new(join_hover_types(left_inner, right_inner)))
        }
        (TypeRef::Nullable(left_inner), TypeRef::Nullable(right_inner)) => {
            TypeRef::Nullable(Box::new(join_hover_types(left_inner, right_inner)))
        }
        (TypeRef::Function(left_sig), TypeRef::Function(right_sig))
            if left_sig.params.len() == right_sig.params.len() =>
        {
            TypeRef::Function(rhai_hir::FunctionTypeRef {
                params: left_sig
                    .params
                    .iter()
                    .zip(right_sig.params.iter())
                    .map(|(left, right)| join_hover_types(left, right))
                    .collect(),
                ret: Box::new(join_hover_types(
                    left_sig.ret.as_ref(),
                    right_sig.ret.as_ref(),
                )),
            })
        }
        _ => merge_hover_union(left, right),
    }
}

fn merge_hover_union(left: &TypeRef, right: &TypeRef) -> TypeRef {
    let mut members = Vec::<TypeRef>::new();
    push_hover_union_member(&mut members, left);
    push_hover_union_member(&mut members, right);
    if members.len() == 1 {
        members.pop().expect("expected one hover union member")
    } else {
        TypeRef::Union(members)
    }
}

fn push_hover_union_member(members: &mut Vec<TypeRef>, ty: &TypeRef) {
    match ty {
        TypeRef::Union(items) | TypeRef::Ambiguous(items) => {
            for item in items {
                push_hover_union_member(members, item);
            }
        }
        other => {
            if members.iter().any(|existing| existing == other) {
                return;
            }
            members.push(other.clone());
        }
    }
}

fn call_targets_symbol(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &rhai_hir::FileHir,
    call: &rhai_hir::CallSite,
    target: &FileBackedSymbolIdentity,
) -> bool {
    call_target_items(snapshot, file_id, hir, call)
        .into_iter()
        .any(|item| item.symbol == *target)
}

fn call_target_items(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &rhai_hir::FileHir,
    call: &rhai_hir::CallSite,
) -> Vec<LocatedCallHierarchyItem> {
    let mut items = Vec::new();

    if let Some(callee) = call.resolved_callee
        && hir.symbol(callee).kind == rhai_hir::SymbolKind::Function
    {
        let identity = hir.file_backed_symbol_identity(callee);
        if let Some(item) = snapshot.call_hierarchy_item_from_identity(&identity) {
            items.push(item);
        }
    }

    if let Some(callee_range) = call.callee_range {
        items.extend(
            hir.references
                .iter()
                .enumerate()
                .filter_map(|(index, reference)| {
                    callee_range
                        .contains_range(reference.range)
                        .then_some(rhai_hir::ReferenceId(index as u32))
                })
                .flat_map(|reference_id| {
                    linked_import_targets_for_path_reference(snapshot, file_id, hir, reference_id)
                })
                .filter(|target| target.symbol.kind == rhai_hir::SymbolKind::Function)
                .filter_map(|target| snapshot.call_hierarchy_item_from_identity(&target.symbol)),
        );
    }

    if let Some(reference_id) = call.callee_reference {
        items.extend(
            linked_import_targets_for_path_reference(snapshot, file_id, hir, reference_id)
                .into_iter()
                .filter(|target| target.symbol.kind == rhai_hir::SymbolKind::Function)
                .filter_map(|target| snapshot.call_hierarchy_item_from_identity(&target.symbol)),
        );

        if hir.reference(reference_id).kind == rhai_hir::ReferenceKind::Field
            && let Some(access) = hir
                .member_accesses
                .iter()
                .find(|access| access.field_reference == reference_id)
            && let Some(receiver_ty) = snapshot
                .type_inference(file_id)
                .and_then(|inference| hir.expr_type(access.receiver, &inference.expr_types))
                .cloned()
                .or_else(|| {
                    snapshot
                        .inferred_expr_type_at(file_id, hir.expr(access.receiver).range.start())
                        .cloned()
                })
        {
            items.extend(
                imported_global_method_symbols(
                    snapshot,
                    file_id,
                    &receiver_ty,
                    hir.reference(reference_id).name.as_str(),
                )
                .into_iter()
                .filter(|target| target.symbol.kind == rhai_hir::SymbolKind::Function)
                .filter_map(|target| snapshot.call_hierarchy_item_from_identity(&target.symbol)),
            );
        }
    }

    items.sort_by(|left, right| {
        left.file_id
            .0
            .cmp(&right.file_id.0)
            .then_with(|| {
                left.target
                    .full_range
                    .start()
                    .cmp(&right.target.full_range.start())
            })
            .then_with(|| left.symbol.name.cmp(&right.symbol.name))
    });
    items.dedup_by(|left, right| left.file_id == right.file_id && left.symbol == right.symbol);
    items
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

fn resolve_unresolved_name_in_outer_scope(
    hir: &FileHir,
    reference_id: ReferenceId,
) -> Option<SymbolId> {
    let reference = hir.reference(reference_id);
    if reference.kind != rhai_hir::ReferenceKind::Name || reference.target.is_some() {
        return None;
    }

    let function_scope = enclosing_function_scope(hir, reference.scope)?;
    let capture = resolve_name_in_outer_scopes(
        hir,
        hir.scope(function_scope).parent?,
        reference.name.as_str(),
        reference.range.start(),
    )?;

    if matches!(
        hir.symbol(capture).kind,
        SymbolKind::Function | SymbolKind::ImportAlias | SymbolKind::ExportAlias
    ) {
        return None;
    }

    Some(capture)
}

fn unresolved_outer_scope_references_to_symbol(
    hir: &FileHir,
    target_symbol: SymbolId,
) -> Vec<ReferenceId> {
    let target = hir.symbol(target_symbol);
    hir.references
        .iter()
        .enumerate()
        .filter_map(|(index, reference)| {
            if reference.kind != rhai_hir::ReferenceKind::Name
                || reference.target.is_some()
                || reference.name != target.name
            {
                return None;
            }
            let reference_id = ReferenceId(index as u32);
            (resolve_unresolved_name_in_outer_scope(hir, reference_id) == Some(target_symbol))
                .then_some(reference_id)
        })
        .collect()
}

fn enclosing_function_scope(hir: &FileHir, mut scope: ScopeId) -> Option<ScopeId> {
    loop {
        let scope_data = hir.scope(scope);
        if scope_data.kind == rhai_hir::ScopeKind::Function {
            return Some(scope);
        }
        scope = scope_data.parent?;
    }
}

fn resolve_name_in_outer_scopes(
    hir: &FileHir,
    mut scope: ScopeId,
    name: &str,
    reference_start: TextSize,
) -> Option<SymbolId> {
    loop {
        if let Some(symbol) = hir
            .scope(scope)
            .symbols
            .iter()
            .rev()
            .copied()
            .find(|symbol_id| {
                let symbol = hir.symbol(*symbol_id);
                symbol.name == name && symbol.range.start() <= reference_start
            })
        {
            return Some(symbol);
        }
        scope = hir.scope(scope).parent?;
    }
}
