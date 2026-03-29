use rhai_syntax::TextRange;

use crate::model::expr::ExprId;
use crate::model::flow::{ControlFlowEvent, ControlFlowMergePoint};
use crate::model::symbol::SymbolId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReferenceId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeKind {
    File,
    Function,
    Block,
    Catch,
    SwitchArm,
    Closure,
    Loop,
    Interpolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceKind {
    Name,
    This,
    PathSegment,
    Field,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyKind {
    Function,
    Closure,
    Block,
    Interpolation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub kind: ScopeKind,
    pub range: TextRange,
    pub parent: Option<ScopeId>,
    pub children: Vec<ScopeId>,
    pub symbols: Vec<SymbolId>,
    pub references: Vec<ReferenceId>,
    pub bodies: Vec<BodyId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Reference {
    pub name: String,
    pub kind: ReferenceKind,
    pub range: TextRange,
    pub scope: ScopeId,
    pub target: Option<SymbolId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body {
    pub kind: BodyKind,
    pub range: TextRange,
    pub scope: ScopeId,
    pub owner: Option<SymbolId>,
    pub control_flow: Vec<ControlFlowEvent>,
    pub return_values: Vec<ExprId>,
    pub throw_values: Vec<ExprId>,
    pub tail_value: Option<ExprId>,
    pub merge_points: Vec<ControlFlowMergePoint>,
    pub may_fall_through: bool,
    pub unreachable_ranges: Vec<TextRange>,
}
