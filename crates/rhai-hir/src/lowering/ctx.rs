use crate::docs::{DocBlock, DocBlockId, DocTag, collect_doc_block};
use crate::model::{
    Body, BodyId, BodyKind, CallSite, CallSiteId, ExprId, ExprKind, ExprNode, FileHir,
    MutationPathSegment, Reference, ReferenceId, ReferenceKind, Scope, ScopeId, ScopeKind, Symbol,
    SymbolId, SymbolKind, TypeSlot, TypeSlotId, ValueFlowKind,
};
use crate::ty::{FunctionTypeRef, TypeRef};
use rhai_syntax::{AstNode, Expr, Parse, TextRange};

pub(crate) struct LoweringContext<'a> {
    pub(crate) parse: &'a Parse,
    pub(crate) file: FileHir,
    pub(crate) body_stack: Vec<BodyId>,
    pub(crate) loop_stack: Vec<ScopeId>,
    pub(crate) pending_value_flows: Vec<PendingValueFlow>,
    pub(crate) pending_mutations: Vec<PendingMutation>,
    pub(crate) pending_reads: Vec<PendingRead>,
}

pub(crate) struct PendingValueFlow {
    pub(crate) reference: ReferenceId,
    pub(crate) expr: ExprId,
    pub(crate) kind: ValueFlowKind,
    pub(crate) range: TextRange,
}

pub(crate) struct PendingMutation {
    pub(crate) receiver_reference: ReferenceId,
    pub(crate) value: ExprId,
    pub(crate) kind: PendingMutationKind,
    pub(crate) range: TextRange,
}

pub(crate) enum PendingMutationKind {
    Path { segments: Vec<MutationPathSegment> },
}

pub(crate) struct PendingRead {
    pub(crate) owner: ExprId,
    pub(crate) root_reference: ReferenceId,
    pub(crate) segments: Vec<MutationPathSegment>,
    pub(crate) range: TextRange,
}

impl<'a> LoweringContext<'a> {
    pub(crate) fn new(parse: &'a Parse) -> Self {
        Self {
            parse,
            file: FileHir {
                root_range: parse.root().text_range(),
                scopes: Vec::new(),
                symbols: Vec::new(),
                references: Vec::new(),
                bodies: Vec::new(),
                exprs: Vec::new(),
                literals: Vec::new(),
                array_exprs: Vec::new(),
                block_exprs: Vec::new(),
                if_exprs: Vec::new(),
                switch_exprs: Vec::new(),
                switch_arms: Vec::new(),
                closure_exprs: Vec::new(),
                path_exprs: Vec::new(),
                for_exprs: Vec::new(),
                function_infos: Vec::new(),
                unary_exprs: Vec::new(),
                binary_exprs: Vec::new(),
                assign_exprs: Vec::new(),
                index_exprs: Vec::new(),
                type_slots: Vec::new(),
                value_flows: Vec::new(),
                symbol_mutations: Vec::new(),
                symbol_reads: Vec::new(),
                calls: Vec::new(),
                expected_type_sites: Vec::new(),
                object_fields: Vec::new(),
                member_accesses: Vec::new(),
                imports: Vec::new(),
                exports: Vec::new(),
                docs: Vec::new(),
            },
            body_stack: Vec::new(),
            loop_stack: Vec::new(),
            pending_value_flows: Vec::new(),
            pending_mutations: Vec::new(),
            pending_reads: Vec::new(),
        }
    }

    pub(crate) fn first_name_reference_from(
        &self,
        start: usize,
        range: TextRange,
    ) -> Option<ReferenceId> {
        self.file.references[start..]
            .iter()
            .enumerate()
            .find_map(|(offset, reference)| {
                (reference.kind == ReferenceKind::Name
                    && reference.range.start() >= range.start()
                    && reference.range.end() <= range.end())
                .then_some(ReferenceId((start + offset) as u32))
            })
    }

    pub(crate) fn first_reference_from(
        &self,
        start: usize,
        range: TextRange,
    ) -> Option<ReferenceId> {
        self.file.references[start..]
            .iter()
            .enumerate()
            .find_map(|(offset, reference)| {
                (reference.range.start() >= range.start() && reference.range.end() <= range.end())
                    .then_some(ReferenceId((start + offset) as u32))
            })
    }

    pub(crate) fn simple_receiver_reference_from(
        &self,
        start: usize,
        expr: &Expr,
    ) -> Option<ReferenceId> {
        match expr {
            Expr::Name(_) => self.first_name_reference_from(start, expr.syntax().text_range()),
            Expr::Paren(paren) => paren
                .expr()
                .and_then(|inner| self.simple_receiver_reference_from(start, &inner)),
            _ => None,
        }
    }

    pub(crate) fn expr_id_for_range(&self, range: TextRange) -> Option<ExprId> {
        self.file
            .exprs
            .iter()
            .enumerate()
            .find_map(|(index, expr)| (expr.range == range).then_some(ExprId(index as u32)))
    }

    pub(crate) fn mutation_target_from_expr(
        &self,
        start: usize,
        expr: &Expr,
    ) -> Option<(ReferenceId, Vec<MutationPathSegment>)> {
        match expr {
            Expr::Field(field) => {
                let name = field.name_token()?.text().to_owned();
                let receiver = field.receiver()?;

                if let Some((reference, mut segments)) =
                    self.mutation_target_from_expr(start, &receiver)
                {
                    segments.push(MutationPathSegment::Field { name });
                    return Some((reference, segments));
                }

                let reference = self.simple_receiver_reference_from(start, &receiver)?;
                Some((reference, vec![MutationPathSegment::Field { name }]))
            }
            Expr::Index(index) => {
                let receiver = index.receiver()?;
                let owner = self.expr_id_for_range(index.syntax().text_range())?;
                let index_expr = self
                    .file
                    .index_exprs
                    .iter()
                    .find(|entry| entry.owner == owner)?
                    .index?;

                if let Some((reference, mut segments)) =
                    self.mutation_target_from_expr(start, &receiver)
                {
                    segments.push(MutationPathSegment::Index { index: index_expr });
                    return Some((reference, segments));
                }

                let reference = self.simple_receiver_reference_from(start, &receiver)?;
                Some((
                    reference,
                    vec![MutationPathSegment::Index { index: index_expr }],
                ))
            }
            Expr::Paren(paren) => paren
                .expr()
                .and_then(|inner| self.mutation_target_from_expr(start, &inner)),
            _ => None,
        }
    }

    pub(crate) fn read_target_for_expr(
        &self,
        expr: ExprId,
    ) -> Option<(ReferenceId, Vec<MutationPathSegment>)> {
        match self.file.expr(expr).kind {
            ExprKind::Field => {
                let access = self.file.member_access(expr)?;
                let (reference, mut segments) = self.read_target_for_receiver(access.receiver)?;
                segments.push(MutationPathSegment::Field {
                    name: self.file.reference(access.field_reference).name.clone(),
                });
                Some((reference, segments))
            }
            ExprKind::Index => {
                let index = self.file.index_expr(expr)?;
                let receiver = index.receiver?;
                let index_expr = index.index?;
                let (reference, mut segments) = self.read_target_for_receiver(receiver)?;
                segments.push(MutationPathSegment::Index { index: index_expr });
                Some((reference, segments))
            }
            _ => None,
        }
    }

    fn read_target_for_receiver(
        &self,
        expr: ExprId,
    ) -> Option<(ReferenceId, Vec<MutationPathSegment>)> {
        match self.file.expr(expr).kind {
            ExprKind::Name => self
                .file
                .reference_at(self.file.expr(expr).range)
                .map(|reference| (reference, Vec::new())),
            ExprKind::Field | ExprKind::Index => self.read_target_for_expr(expr),
            ExprKind::Paren => self
                .largest_inner_expr(expr)
                .and_then(|inner| self.read_target_for_receiver(inner)),
            _ => None,
        }
    }

    fn largest_inner_expr(&self, expr: ExprId) -> Option<ExprId> {
        let range = self.file.expr(expr).range;
        self.file
            .exprs
            .iter()
            .enumerate()
            .filter(|(index, node)| {
                let candidate = ExprId(*index as u32);
                candidate != expr
                    && node.range.start() >= range.start()
                    && node.range.end() <= range.end()
                    && node.range != range
            })
            .max_by_key(|(_, node)| node.range.len())
            .map(|(index, _)| ExprId(index as u32))
    }

    pub(crate) fn new_scope(
        &mut self,
        kind: ScopeKind,
        range: TextRange,
        parent: Option<ScopeId>,
    ) -> ScopeId {
        let id = ScopeId(self.file.scopes.len() as u32);
        self.file.scopes.push(Scope {
            kind,
            range,
            parent,
            children: Vec::new(),
            symbols: Vec::new(),
            references: Vec::new(),
            bodies: Vec::new(),
        });

        if let Some(parent) = parent {
            self.file.scopes[parent.0 as usize].children.push(id);
        }

        id
    }

    pub(crate) fn new_body(
        &mut self,
        kind: BodyKind,
        range: TextRange,
        scope: ScopeId,
        owner: Option<SymbolId>,
    ) -> BodyId {
        let id = BodyId(self.file.bodies.len() as u32);
        self.file.bodies.push(Body {
            kind,
            range,
            scope,
            owner,
            control_flow: Vec::new(),
            return_values: Vec::new(),
            throw_values: Vec::new(),
            tail_value: None,
            merge_points: Vec::new(),
            may_fall_through: true,
            unreachable_ranges: Vec::new(),
        });
        self.file.scopes[scope.0 as usize].bodies.push(id);
        id
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_call(
        &mut self,
        range: TextRange,
        scope: ScopeId,
        caller_scope: bool,
        callee_range: Option<TextRange>,
        callee_reference: Option<ReferenceId>,
        arg_ranges: Vec<TextRange>,
        arg_exprs: Vec<ExprId>,
    ) -> CallSiteId {
        let id = CallSiteId(self.file.calls.len() as u32);
        self.file.calls.push(CallSite {
            range,
            scope,
            caller_scope,
            callee_range,
            callee_reference,
            resolved_callee: None,
            arg_ranges,
            arg_exprs,
            parameter_bindings: Vec::new(),
        });
        id
    }

    pub(crate) fn alloc_expr(
        &mut self,
        kind: ExprKind,
        range: TextRange,
        scope: ScopeId,
    ) -> ExprId {
        let result_slot = TypeSlotId(self.file.type_slots.len() as u32);
        self.file.type_slots.push(TypeSlot { range });
        let id = ExprId(self.file.exprs.len() as u32);
        self.file.exprs.push(ExprNode {
            kind,
            range,
            scope,
            result_slot,
        });
        id
    }

    pub(crate) fn alloc_doc_block(&mut self, doc: DocBlock) -> DocBlockId {
        let id = DocBlockId(self.file.docs.len() as u32);
        self.file.docs.push(doc);
        id
    }

    pub(crate) fn docs_for_range(&mut self, range: TextRange) -> Option<DocBlockId> {
        let doc = collect_doc_block(&self.parse.root(), range.start())?;
        Some(self.alloc_doc_block(doc))
    }

    pub(crate) fn text_for_range(&self, range: TextRange) -> String {
        let start: u32 = range.start().into();
        let end: u32 = range.end().into();
        self.parse
            .text()
            .get(start as usize..end as usize)
            .unwrap_or("")
            .to_owned()
    }

    pub(crate) fn doc_block(&self, docs: Option<DocBlockId>) -> Option<&DocBlock> {
        let docs = docs?;
        Some(self.file.doc_block(docs))
    }

    pub(crate) fn annotation_from_docs(&self, docs: Option<DocBlockId>) -> Option<TypeRef> {
        self.doc_block(docs)?.tags.iter().find_map(|tag| match tag {
            DocTag::Type(ty) => Some(ty.clone()),
            _ => None,
        })
    }

    pub(crate) fn param_annotation_from_docs(
        &self,
        docs: Option<DocBlockId>,
        name: &str,
    ) -> Option<TypeRef> {
        self.doc_block(docs)?.tags.iter().find_map(|tag| match tag {
            DocTag::Param { name: param, ty } if param == name => Some(ty.clone()),
            _ => None,
        })
    }

    pub(crate) fn function_annotation_from_docs(
        &self,
        docs: Option<DocBlockId>,
        params: &[String],
    ) -> Option<TypeRef> {
        if let Some(annotation) = self.annotation_from_docs(docs) {
            return Some(annotation);
        }

        let docs = self.doc_block(docs)?;
        let has_signature_tags = docs
            .tags
            .iter()
            .any(|tag| matches!(tag, DocTag::Param { .. } | DocTag::Return(_)));

        if !has_signature_tags {
            return None;
        }

        let params = params
            .iter()
            .map(|name| {
                docs.tags
                    .iter()
                    .find_map(|tag| match tag {
                        DocTag::Param { name: param, ty } if param == name => Some(ty.clone()),
                        _ => None,
                    })
                    .unwrap_or(TypeRef::Unknown)
            })
            .collect();
        let ret = docs
            .tags
            .iter()
            .find_map(|tag| match tag {
                DocTag::Return(ty) => Some(ty.clone()),
                _ => None,
            })
            .unwrap_or(TypeRef::Unknown);

        Some(TypeRef::Function(FunctionTypeRef {
            params,
            ret: Box::new(ret),
        }))
    }

    pub(crate) fn alloc_symbol(
        &mut self,
        name: String,
        kind: SymbolKind,
        range: TextRange,
        scope: ScopeId,
        docs: Option<DocBlockId>,
    ) -> SymbolId {
        let annotation = self.annotation_from_docs(docs);
        self.alloc_symbol_with_annotation(name, kind, false, range, scope, docs, annotation)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn alloc_symbol_with_annotation(
        &mut self,
        name: String,
        kind: SymbolKind,
        is_private: bool,
        range: TextRange,
        scope: ScopeId,
        docs: Option<DocBlockId>,
        annotation: Option<TypeRef>,
    ) -> SymbolId {
        let id = SymbolId(self.file.symbols.len() as u32);
        self.file.symbols.push(Symbol {
            name,
            kind,
            is_private,
            range,
            scope,
            docs,
            annotation,
            references: Vec::new(),
            shadowed: None,
            duplicate_of: None,
        });
        self.file.scopes[scope.0 as usize].symbols.push(id);
        id
    }

    pub(crate) fn alloc_reference(
        &mut self,
        name: String,
        kind: ReferenceKind,
        range: TextRange,
        scope: ScopeId,
    ) -> ReferenceId {
        let id = ReferenceId(self.file.references.len() as u32);
        self.file.references.push(Reference {
            name,
            kind,
            range,
            scope,
            target: None,
        });
        self.file.scopes[scope.0 as usize].references.push(id);
        id
    }
}
