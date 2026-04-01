use crate::db::DatabaseSnapshot;
use crate::db::imports::linked_import_targets_for_path_reference;
use crate::db::rename::project_reference_kind_rank;
use crate::infer::{field_value_exprs_from_expr, field_value_exprs_from_symbol};
use crate::types::{
    LocatedNavigationTarget, LocatedProjectReference, ObjectFieldHoverInfo, ProjectReferenceKind,
    ProjectReferences,
};
use rhai_hir::{FileHir, ReferenceId, SymbolId, SymbolKind, TypeRef};
use rhai_syntax::{TextRange, TextSize};
use rhai_vfs::FileId;
use std::collections::BTreeMap;

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
    pub(crate) fn range(&self) -> TextRange {
        TextRange::new(self.key.start, self.key.end)
    }
}

impl DatabaseSnapshot {
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

    pub(crate) fn object_field_goto_targets(
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

    pub(crate) fn object_field_project_references(
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
