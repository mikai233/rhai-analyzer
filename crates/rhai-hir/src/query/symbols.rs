use rhai_syntax::{TextRange, TextSize};

use crate::docs::{DocBlock, DocBlockId, DocTag};
use crate::model::{
    ArrayExprInfo, AssignExprInfo, BinaryExprInfo, BlockExprInfo, BodyId, ClosureExprInfo,
    ControlFlowEvent, ControlFlowMergePoint, DocumentedField, ExportDirective, ExprId, FileHir,
    FindReferencesResult, IfExprInfo, ImportDirective, IndexExprInfo, LiteralInfo, MemberAccess,
    NavigationTarget, PathExprInfo, ReferenceId, ReferenceLocation, SwitchExprInfo, SymbolId,
    TypeSlotId, UnaryExprInfo,
};
use crate::ty::TypeRef;

impl FileHir {
    pub fn literal(&self, expr: ExprId) -> Option<&LiteralInfo> {
        self.literals.iter().find(|literal| literal.owner == expr)
    }

    pub fn array_expr(&self, expr: ExprId) -> Option<&ArrayExprInfo> {
        self.array_exprs.iter().find(|array| array.owner == expr)
    }

    pub fn block_expr(&self, expr: ExprId) -> Option<&BlockExprInfo> {
        self.block_exprs.iter().find(|block| block.owner == expr)
    }

    pub fn if_expr(&self, expr: ExprId) -> Option<&IfExprInfo> {
        self.if_exprs.iter().find(|if_expr| if_expr.owner == expr)
    }

    pub fn while_expr(&self, expr: ExprId) -> Option<&crate::WhileExprInfo> {
        self.while_exprs
            .iter()
            .find(|while_expr| while_expr.owner == expr)
    }

    pub fn do_expr(&self, expr: ExprId) -> Option<&crate::DoExprInfo> {
        self.do_exprs.iter().find(|do_expr| do_expr.owner == expr)
    }

    pub fn switch_expr(&self, expr: ExprId) -> Option<&SwitchExprInfo> {
        self.switch_exprs
            .iter()
            .find(|switch_expr| switch_expr.owner == expr)
    }

    pub fn switch_arms(&self, expr: ExprId) -> impl Iterator<Item = &crate::SwitchArmInfo> + '_ {
        self.switch_arms
            .iter()
            .filter(move |switch_arm| switch_arm.owner == expr)
    }

    pub fn closure_expr(&self, expr: ExprId) -> Option<&ClosureExprInfo> {
        self.closure_exprs
            .iter()
            .find(|closure| closure.owner == expr)
    }

    pub fn path_expr(&self, expr: ExprId) -> Option<&PathExprInfo> {
        self.path_exprs.iter().find(|path| path.owner == expr)
    }

    pub fn for_expr(&self, expr: ExprId) -> Option<&crate::ForExprInfo> {
        self.for_exprs
            .iter()
            .find(|for_expr| for_expr.owner == expr)
    }

    pub fn function_info(&self, function: SymbolId) -> Option<&crate::FunctionInfo> {
        self.function_infos
            .iter()
            .find(|info| info.symbol == function)
    }

    pub fn unary_expr(&self, expr: ExprId) -> Option<&UnaryExprInfo> {
        self.unary_exprs.iter().find(|unary| unary.owner == expr)
    }

    pub fn binary_expr(&self, expr: ExprId) -> Option<&BinaryExprInfo> {
        self.binary_exprs.iter().find(|binary| binary.owner == expr)
    }

    pub fn assign_expr(&self, expr: ExprId) -> Option<&AssignExprInfo> {
        self.assign_exprs.iter().find(|assign| assign.owner == expr)
    }

    pub fn index_expr(&self, expr: ExprId) -> Option<&IndexExprInfo> {
        self.index_exprs.iter().find(|index| index.owner == expr)
    }

    pub fn member_access(&self, expr: ExprId) -> Option<&MemberAccess> {
        self.member_accesses
            .iter()
            .find(|access| access.owner == expr)
    }

    pub fn body_of(&self, owner: SymbolId) -> Option<BodyId> {
        self.bodies
            .iter()
            .position(|body| body.owner == Some(owner))
            .map(|index| BodyId(index as u32))
    }

    pub fn body_control_flow(&self, body: BodyId) -> impl Iterator<Item = &ControlFlowEvent> + '_ {
        self.body(body).control_flow.iter()
    }

    pub fn body_return_values(&self, body: BodyId) -> impl Iterator<Item = ExprId> + '_ {
        self.body(body).return_values.iter().copied()
    }

    pub fn body_throw_values(&self, body: BodyId) -> impl Iterator<Item = ExprId> + '_ {
        self.body(body).throw_values.iter().copied()
    }

    pub fn body_tail_value(&self, body: BodyId) -> Option<ExprId> {
        self.body(body).tail_value
    }

    pub fn body_merge_points(
        &self,
        body: BodyId,
    ) -> impl Iterator<Item = &ControlFlowMergePoint> + '_ {
        self.body(body).merge_points.iter()
    }

    pub fn body_may_fall_through(&self, body: BodyId) -> bool {
        self.body(body).may_fall_through
    }

    pub fn body_unreachable_ranges(&self, body: BodyId) -> impl Iterator<Item = TextRange> + '_ {
        self.body(body).unreachable_ranges.iter().copied()
    }

    pub fn import(&self, index: usize) -> &ImportDirective {
        &self.imports[index]
    }

    pub fn export(&self, index: usize) -> &ExportDirective {
        &self.exports[index]
    }

    pub fn doc_block(&self, id: DocBlockId) -> &DocBlock {
        &self.docs[id.0 as usize]
    }

    pub fn documented_fields(&self, symbol: SymbolId) -> Vec<DocumentedField> {
        let Some(doc_id) = self.symbol(symbol).docs else {
            return Vec::new();
        };

        self.doc_block(doc_id)
            .tags
            .iter()
            .filter_map(|tag| match tag {
                DocTag::Field { name, ty, docs } => Some(DocumentedField {
                    name: name.clone(),
                    annotation: ty.clone(),
                    docs: docs.clone(),
                }),
                _ => None,
            })
            .collect()
    }

    pub fn expr_result_slot_at_offset(&self, offset: TextSize) -> Option<TypeSlotId> {
        let expr = self.expr_at_offset(offset)?;
        Some(self.expr_result_slot(expr))
    }

    pub fn references_to(&self, symbol: SymbolId) -> impl Iterator<Item = ReferenceId> + '_ {
        self.symbol(symbol).references.iter().copied()
    }

    pub fn definition_of(&self, reference: ReferenceId) -> Option<SymbolId> {
        self.reference(reference).target
    }

    pub fn definition_at_offset(&self, offset: TextSize) -> Option<SymbolId> {
        if let Some(reference) = self.reference_at_offset(offset) {
            return self.definition_of(reference);
        }

        self.symbol_at_offset(offset)
    }

    pub fn goto_definition(&self, offset: TextSize) -> Option<NavigationTarget> {
        let symbol = self.definition_at_offset(offset)?;
        Some(self.navigation_target(symbol))
    }

    pub fn find_references(&self, offset: TextSize) -> Option<FindReferencesResult> {
        let symbol = if let Some(reference) = self.reference_at_offset(offset) {
            self.definition_of(reference)?
        } else {
            self.symbol_at_offset(offset)?
        };

        let declaration = self.navigation_target(symbol);
        let references = self
            .references_to(symbol)
            .map(|reference_id| {
                let reference = self.reference(reference_id);
                ReferenceLocation {
                    reference: reference_id,
                    kind: reference.kind,
                    range: reference.range,
                    target: symbol,
                }
            })
            .collect();

        Some(FindReferencesResult {
            symbol,
            declaration,
            references,
        })
    }

    pub(crate) fn symbol_for_expr(&self, expr: ExprId) -> Option<SymbolId> {
        match self.expr(expr).kind {
            crate::ExprKind::Name => self
                .reference_at(self.expr(expr).range)
                .and_then(|reference| self.definition_of(reference)),
            _ => None,
        }
    }

    pub(crate) fn object_field_annotation_from_expr(&self, expr: ExprId) -> Option<TypeRef> {
        match self.expr(expr).kind {
            crate::ExprKind::Literal => self.literal(expr).map(|literal| match literal.kind {
                crate::LiteralKind::Int => TypeRef::Int,
                crate::LiteralKind::Float => TypeRef::Float,
                crate::LiteralKind::String => TypeRef::String,
                crate::LiteralKind::Char => TypeRef::Char,
                crate::LiteralKind::Bool => TypeRef::Bool,
            }),
            crate::ExprKind::Object => Some(TypeRef::Object(
                self.object_fields
                    .iter()
                    .filter(|field| field.owner == expr)
                    .map(|field| {
                        (
                            field.name.clone(),
                            field
                                .value
                                .and_then(|value| self.object_field_annotation_from_expr(value))
                                .unwrap_or(TypeRef::Unknown),
                        )
                    })
                    .collect(),
            )),
            crate::ExprKind::Array => Some(TypeRef::Array(Box::new(TypeRef::Unknown))),
            crate::ExprKind::Closure => Some(TypeRef::Function(crate::FunctionTypeRef {
                params: Vec::new(),
                ret: Box::new(TypeRef::Unknown),
            })),
            crate::ExprKind::Name => self
                .symbol_for_expr(expr)
                .and_then(|symbol| self.declared_symbol_type(symbol).cloned()),
            _ => None,
        }
    }
}
