use rhai_syntax::TextRange;

use crate::model::expr::ExprId;
use crate::model::scope::ScopeId;
use crate::model::symbol::SymbolId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlFlowKind {
    Return,
    Throw,
    Break,
    Continue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MergePointKind {
    IfElse,
    Switch,
    LoopIteration,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ControlFlowEvent {
    pub kind: ControlFlowKind,
    pub range: TextRange,
    pub value_range: Option<TextRange>,
    pub target_loop: Option<ScopeId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ControlFlowMergePoint {
    pub kind: MergePointKind,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueFlowKind {
    Initializer,
    Assignment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolValueFlow {
    pub symbol: SymbolId,
    pub expr: ExprId,
    pub kind: ValueFlowKind,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationPathSegment {
    Field { name: String },
    Index { index: ExprId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolMutationKind {
    Path { segments: Vec<MutationPathSegment> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolMutation {
    pub symbol: SymbolId,
    pub value: ExprId,
    pub kind: SymbolMutationKind,
    pub range: TextRange,
}
