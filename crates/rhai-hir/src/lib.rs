mod diagnostics;
mod docs;
mod indexing;
mod lowering;
mod model;
mod query;
mod rename;
#[cfg(test)]
mod tests;
mod ty;

pub use docs::{DocBlock, DocBlockId, DocTag, collect_doc_block};
pub use lowering::lower_file;
pub use model::{
    ArrayExprInfo, AssignExprInfo, AssignmentOperator, BinaryExprInfo, BinaryOperator,
    BlockExprInfo, Body, BodyId, BodyKind, CallSite, CallSiteId, ClosureExprInfo, CompletionSymbol,
    ControlFlowEvent, ControlFlowKind, ControlFlowMergePoint, DocumentSymbol, DocumentedField,
    ExportDirective, ExprId, ExprKind, ExprNode, ExternalSignatureIndex, FileBackedSymbolIdentity,
    FileHir, FileSymbolId, FileSymbolIndex, FileSymbolIndexEntry, FindReferencesResult,
    ForExprInfo, IfExprInfo, ImportDirective, IndexExprInfo, IndexableSymbol, IndexingHandoff,
    LinkedAlias, LinkedAliasKind, LiteralInfo, LiteralKind, MemberAccess, MemberCompletion,
    MemberCompletionSource, MergePointKind, ModuleExportEdge, ModuleGraphIndex, ModuleImportEdge,
    ModuleSpecifier, MutationPathSegment, NavigationTarget, ObjectFieldInfo, ParameterHint,
    ParameterHintParameter, Reference, ReferenceId, ReferenceKind, ReferenceLocation,
    RenameOccurrence, RenameOccurrenceKind, RenamePlan, RenamePreflightIssue,
    RenamePreflightIssueKind, Scope, ScopeId, ScopeKind, SemanticDiagnostic,
    SemanticDiagnosticKind, StableSymbolKey, SwitchExprInfo, Symbol, SymbolId, SymbolKind,
    SymbolMutation, SymbolMutationKind, SymbolValueFlow, TypeSlot, TypeSlotAssignments, TypeSlotId,
    UnaryExprInfo, UnaryOperator, ValueFlowKind, WorkspaceSymbol,
};
pub use ty::{FunctionTypeRef, TypeRef, parse_type_ref};

use rhai_syntax::TextSize;

pub type LoweredFile = FileHir;

impl FileHir {
    pub fn scope(&self, id: ScopeId) -> &Scope {
        &self.scopes[id.0 as usize]
    }

    pub fn symbol(&self, id: SymbolId) -> &Symbol {
        &self.symbols[id.0 as usize]
    }

    pub fn shadowed_symbol_of(&self, symbol: SymbolId) -> Option<SymbolId> {
        self.symbol(symbol).shadowed
    }

    pub fn duplicate_definition_of(&self, symbol: SymbolId) -> Option<SymbolId> {
        self.symbol(symbol).duplicate_of
    }

    pub fn reference(&self, id: ReferenceId) -> &Reference {
        &self.references[id.0 as usize]
    }

    pub fn navigation_target(&self, symbol: SymbolId) -> NavigationTarget {
        let symbol_data = self.symbol(symbol);
        NavigationTarget {
            symbol,
            kind: symbol_data.kind,
            full_range: symbol_data.range,
            focus_range: symbol_data.range,
        }
    }

    pub fn body(&self, id: BodyId) -> &Body {
        &self.bodies[id.0 as usize]
    }

    pub fn expr(&self, id: ExprId) -> &ExprNode {
        &self.exprs[id.0 as usize]
    }

    pub fn type_slot(&self, id: TypeSlotId) -> &TypeSlot {
        &self.type_slots[id.0 as usize]
    }

    pub fn new_type_slot_assignments(&self) -> TypeSlotAssignments {
        TypeSlotAssignments::with_slot_count(self.type_slots.len())
    }

    pub fn expr_result_slot(&self, expr: ExprId) -> TypeSlotId {
        self.expr(expr).result_slot
    }

    pub fn expr_type<'a>(
        &self,
        expr: ExprId,
        assignments: &'a TypeSlotAssignments,
    ) -> Option<&'a TypeRef> {
        assignments.get(self.expr_result_slot(expr))
    }

    pub fn expr_type_at_offset<'a>(
        &self,
        offset: TextSize,
        assignments: &'a TypeSlotAssignments,
    ) -> Option<&'a TypeRef> {
        let expr = self.expr_at_offset(offset)?;
        self.expr_type(expr, assignments)
    }

    pub fn declared_symbol_type(&self, symbol: SymbolId) -> Option<&TypeRef> {
        self.symbol(symbol).annotation.as_ref()
    }

    pub fn effective_symbol_type(
        &self,
        symbol: SymbolId,
        external: Option<&ExternalSignatureIndex>,
    ) -> Option<TypeRef> {
        self.declared_symbol_type(symbol).cloned().or_else(|| {
            external.and_then(|index| index.get(self.symbol(symbol).name.as_str()).cloned())
        })
    }

    pub fn value_flows_into(
        &self,
        symbol: SymbolId,
    ) -> impl Iterator<Item = &SymbolValueFlow> + '_ {
        self.value_flows
            .iter()
            .filter(move |flow| flow.symbol == symbol)
    }

    pub fn symbol_mutations_into(
        &self,
        symbol: SymbolId,
    ) -> impl Iterator<Item = &SymbolMutation> + '_ {
        self.symbol_mutations
            .iter()
            .filter(move |mutation| mutation.symbol == symbol)
    }

    pub fn call(&self, id: CallSiteId) -> &CallSite {
        &self.calls[id.0 as usize]
    }

    pub fn function_parameters(&self, function: SymbolId) -> Vec<SymbolId> {
        let Some(body_id) = self.body_of(function) else {
            return Vec::new();
        };

        self.scope(self.body(body_id).scope)
            .symbols
            .iter()
            .copied()
            .filter(|symbol_id| self.symbol(*symbol_id).kind == SymbolKind::Parameter)
            .collect()
    }

    pub fn call_parameter_binding(
        &self,
        call: CallSiteId,
        argument_index: usize,
    ) -> Option<SymbolId> {
        self.call(call)
            .parameter_bindings
            .get(argument_index)
            .copied()
            .flatten()
    }

    pub fn call_argument_expr(&self, call: CallSiteId, argument_index: usize) -> Option<ExprId> {
        self.call(call).arg_exprs.get(argument_index).copied()
    }

    pub fn call_signature(
        &self,
        call: CallSiteId,
        external: Option<&ExternalSignatureIndex>,
    ) -> Option<FunctionTypeRef> {
        let call = self.call(call);

        if let Some(callee) = call.resolved_callee {
            return match self.effective_symbol_type(callee, external)? {
                TypeRef::Function(signature) => Some(signature),
                _ => None,
            };
        }

        let external = external?;
        let callee_name = call
            .callee_reference
            .map(|reference_id| self.reference(reference_id).name.as_str())?;
        match external.get(callee_name)? {
            TypeRef::Function(signature) => Some(signature.clone()),
            _ => None,
        }
    }
}
