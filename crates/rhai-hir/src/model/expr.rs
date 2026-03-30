use std::collections::BTreeMap;

use rhai_syntax::TextRange;

use crate::model::scope::{BodyId, ReferenceId, ScopeId};
use crate::model::symbol::SymbolId;
use crate::ty::TypeRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExprId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeSlotId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallSiteId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExpectedTypeSource {
    Symbol(SymbolId),
    FunctionReturn(SymbolId),
    CallArgument {
        call: CallSiteId,
        parameter_index: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExpectedTypeSite {
    pub expr: ExprId,
    pub source: ExpectedTypeSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExprKind {
    Name,
    Literal,
    Array,
    Object,
    If,
    Switch,
    While,
    Loop,
    For,
    Do,
    Path,
    Closure,
    InterpolatedString,
    Unary,
    Binary,
    Assign,
    Paren,
    Call,
    Index,
    Field,
    Block,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LiteralKind {
    Int,
    Float,
    String,
    Char,
    Bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOperator {
    Plus,
    Minus,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOperator {
    OrOr,
    Or,
    Xor,
    AndAnd,
    And,
    EqEq,
    NotEq,
    In,
    Gt,
    GtEq,
    Lt,
    LtEq,
    NullCoalesce,
    Range,
    RangeInclusive,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Power,
    ShiftLeft,
    ShiftRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssignmentOperator {
    Assign,
    NullCoalesce,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Power,
    ShiftLeft,
    ShiftRight,
    Or,
    Xor,
    And,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExprNode {
    pub kind: ExprKind,
    pub range: TextRange,
    pub scope: ScopeId,
    pub result_slot: TypeSlotId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LiteralInfo {
    pub owner: ExprId,
    pub kind: LiteralKind,
    pub range: TextRange,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArrayExprInfo {
    pub owner: ExprId,
    pub items: Vec<ExprId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockExprInfo {
    pub owner: ExprId,
    pub body: BodyId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IfExprInfo {
    pub owner: ExprId,
    pub condition: Option<ExprId>,
    pub then_branch: Option<ExprId>,
    pub else_branch: Option<ExprId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SwitchExprInfo {
    pub owner: ExprId,
    pub scrutinee: Option<ExprId>,
    pub arms: Vec<Option<ExprId>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SwitchArmInfo {
    pub owner: ExprId,
    pub scope: ScopeId,
    pub patterns: Vec<ExprId>,
    pub wildcard: bool,
    pub value: Option<ExprId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClosureExprInfo {
    pub owner: ExprId,
    pub body: BodyId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PathExprInfo {
    pub owner: ExprId,
    pub base: Option<ExprId>,
    pub rooted_global: bool,
    pub segments: Vec<ReferenceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForExprInfo {
    pub owner: ExprId,
    pub iterable: Option<ExprId>,
    pub bindings: Vec<SymbolId>,
    pub body: Option<BodyId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionInfo {
    pub symbol: SymbolId,
    pub this_type: Option<TypeRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnaryExprInfo {
    pub owner: ExprId,
    pub operator: UnaryOperator,
    pub operand: Option<ExprId>,
    pub operator_range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BinaryExprInfo {
    pub owner: ExprId,
    pub operator: BinaryOperator,
    pub lhs: Option<ExprId>,
    pub rhs: Option<ExprId>,
    pub operator_range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssignExprInfo {
    pub owner: ExprId,
    pub operator: AssignmentOperator,
    pub lhs: Option<ExprId>,
    pub rhs: Option<ExprId>,
    pub operator_range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IndexExprInfo {
    pub owner: ExprId,
    pub receiver: Option<ExprId>,
    pub index: Option<ExprId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeSlot {
    pub range: TextRange,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeSlotAssignments {
    values: Vec<Option<TypeRef>>,
}

impl TypeSlotAssignments {
    pub fn with_slot_count(slot_count: usize) -> Self {
        Self {
            values: vec![None; slot_count],
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn set(&mut self, slot: TypeSlotId, ty: TypeRef) {
        let index = slot.0 as usize;
        if index >= self.values.len() {
            self.values.resize(index + 1, None);
        }
        self.values[index] = Some(ty);
    }

    pub fn get(&self, slot: TypeSlotId) -> Option<&TypeRef> {
        self.values
            .get(slot.0 as usize)
            .and_then(|value| value.as_ref())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExternalSignatureIndex {
    symbols: BTreeMap<String, TypeRef>,
}

impl ExternalSignatureIndex {
    pub fn insert(&mut self, name: impl Into<String>, ty: TypeRef) -> Option<TypeRef> {
        self.symbols.insert(name.into(), ty)
    }

    pub fn get(&self, name: &str) -> Option<&TypeRef> {
        self.symbols.get(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    pub range: TextRange,
    pub scope: ScopeId,
    pub caller_scope: bool,
    pub callee_range: Option<TextRange>,
    pub callee_reference: Option<ReferenceId>,
    pub resolved_callee: Option<SymbolId>,
    pub arg_ranges: Vec<TextRange>,
    pub arg_exprs: Vec<ExprId>,
    pub parameter_bindings: Vec<Option<SymbolId>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectFieldInfo {
    pub owner: ExprId,
    pub name: String,
    pub range: TextRange,
    pub value: Option<ExprId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberAccess {
    pub owner: ExprId,
    pub range: TextRange,
    pub scope: ScopeId,
    pub receiver: ExprId,
    pub field_reference: ReferenceId,
}
