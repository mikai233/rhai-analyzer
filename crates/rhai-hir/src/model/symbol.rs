use rhai_syntax::TextRange;

use crate::docs::DocBlock;
use crate::docs::DocBlockId;
use crate::model::expr::{
    ArrayExprInfo, AssignExprInfo, BinaryExprInfo, BlockExprInfo, CallSite, CallSiteId,
    ClosureExprInfo, DoExprInfo, ExpectedTypeSite, ExprNode, ForExprInfo, FunctionInfo, IfExprInfo,
    IndexExprInfo, LiteralInfo, MemberAccess, ObjectFieldInfo, PathExprInfo, SwitchArmInfo,
    SwitchExprInfo, TypeSlot, UnaryExprInfo, WhileExprInfo,
};
use crate::model::flow::{SymbolMutation, SymbolRead, SymbolValueFlow};
use crate::model::module::{ExportDirective, ImportDirective, NavigationTarget};
use crate::model::scope::{Body, Reference, ReferenceId, ReferenceKind, Scope, ScopeId};
use crate::ty::TypeRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Variable,
    Parameter,
    Constant,
    Function,
    ImportAlias,
    ExportAlias,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub is_private: bool,
    pub range: TextRange,
    pub scope: ScopeId,
    pub docs: Option<DocBlockId>,
    pub annotation: Option<TypeRef>,
    pub references: Vec<ReferenceId>,
    pub shadowed: Option<SymbolId>,
    pub duplicate_of: Option<SymbolId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SymbolConflictKey {
    pub(crate) name: String,
    pub(crate) function_receiver: Option<TypeRef>,
    pub(crate) function_arity: Option<usize>,
    pub(crate) function_param_types: Option<Vec<TypeRef>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHir {
    pub root_range: TextRange,
    pub scopes: Vec<Scope>,
    pub symbols: Vec<Symbol>,
    pub references: Vec<Reference>,
    pub bodies: Vec<Body>,
    pub exprs: Vec<ExprNode>,
    pub literals: Vec<LiteralInfo>,
    pub array_exprs: Vec<ArrayExprInfo>,
    pub block_exprs: Vec<BlockExprInfo>,
    pub if_exprs: Vec<IfExprInfo>,
    pub while_exprs: Vec<WhileExprInfo>,
    pub do_exprs: Vec<DoExprInfo>,
    pub switch_exprs: Vec<SwitchExprInfo>,
    pub switch_arms: Vec<SwitchArmInfo>,
    pub closure_exprs: Vec<ClosureExprInfo>,
    pub path_exprs: Vec<PathExprInfo>,
    pub for_exprs: Vec<ForExprInfo>,
    pub function_infos: Vec<FunctionInfo>,
    pub unary_exprs: Vec<UnaryExprInfo>,
    pub binary_exprs: Vec<BinaryExprInfo>,
    pub assign_exprs: Vec<AssignExprInfo>,
    pub index_exprs: Vec<IndexExprInfo>,
    pub type_slots: Vec<TypeSlot>,
    pub value_flows: Vec<SymbolValueFlow>,
    pub symbol_mutations: Vec<SymbolMutation>,
    pub symbol_reads: Vec<SymbolRead>,
    pub calls: Vec<CallSite>,
    pub expected_type_sites: Vec<ExpectedTypeSite>,
    pub object_fields: Vec<ObjectFieldInfo>,
    pub member_accesses: Vec<MemberAccess>,
    pub imports: Vec<ImportDirective>,
    pub exports: Vec<ExportDirective>,
    pub docs: Vec<DocBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentedField {
    pub name: String,
    pub annotation: TypeRef,
    pub docs: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionSymbol {
    pub symbol: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub range: TextRange,
    pub scope_distance: u8,
    pub docs: Option<DocBlockId>,
    pub annotation: Option<TypeRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReferenceLocation {
    pub reference: ReferenceId,
    pub kind: ReferenceKind,
    pub range: TextRange,
    pub target: SymbolId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterHintParameter {
    pub symbol: Option<SymbolId>,
    pub name: String,
    pub annotation: Option<TypeRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterHint {
    pub call: CallSiteId,
    pub callee: NavigationTarget,
    pub callee_name: String,
    pub active_parameter: usize,
    pub parameters: Vec<ParameterHintParameter>,
    pub return_type: Option<TypeRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindReferencesResult {
    pub symbol: SymbolId,
    pub declaration: NavigationTarget,
    pub references: Vec<ReferenceLocation>,
}
