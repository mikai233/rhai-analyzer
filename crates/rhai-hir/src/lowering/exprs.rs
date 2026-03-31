use crate::lowering::ctx::{
    LoweringContext, PendingMutation, PendingMutationKind, PendingValueFlow,
};
use crate::model::{
    ArrayExprInfo, AssignExprInfo, AssignmentOperator, BinaryExprInfo, BinaryOperator,
    BlockExprInfo, BodyId, BodyKind, ClosureExprInfo, ExprKind, ForExprInfo, IfExprInfo,
    IndexExprInfo, LiteralInfo, LiteralKind, MemberAccess, MergePointKind, ObjectFieldInfo,
    PathExprInfo, ReferenceKind, ScopeId, ScopeKind, SwitchExprInfo, SymbolKind, UnaryExprInfo,
    UnaryOperator, ValueFlowKind,
};
use rhai_syntax::{
    AstNode, BlockExpr, Expr, Item, Stmt, StringPart, SwitchArm, SwitchPatternList, TokenKind,
};

impl<'a> LoweringContext<'a> {
    pub(crate) fn lower_expr(&mut self, expr: Expr, scope: ScopeId) -> crate::ExprId {
        let expr_id = self.alloc_expr(expr_kind(&expr), expr.syntax().text_range(), scope);

        match expr {
            Expr::Name(name) => {
                if let Some(token) = name.token() {
                    let kind = match token.kind().token_kind() {
                        Some(rhai_syntax::TokenKind::ThisKw) => ReferenceKind::This,
                        _ => ReferenceKind::Name,
                    };
                    self.alloc_reference(token.text().to_owned(), kind, token.text_range(), scope);
                }
            }
            Expr::Literal(literal) => {
                if let Some(token) = literal.token()
                    && let Some(kind) = token.kind().token_kind().and_then(literal_kind)
                {
                    self.file.literals.push(LiteralInfo {
                        owner: expr_id,
                        kind,
                        range: token.text_range(),
                        text: Some(token.text().to_owned()),
                    });
                }
            }
            Expr::Error(_) => {}
            Expr::Array(array) => {
                let mut item_exprs = Vec::new();
                if let Some(items) = array.items() {
                    for item in items.exprs() {
                        item_exprs.push(self.lower_expr(item, scope));
                    }
                }
                self.file.array_exprs.push(ArrayExprInfo {
                    owner: expr_id,
                    items: item_exprs,
                });
            }
            Expr::Object(object) => {
                if let Some(fields) = object.field_list() {
                    for field in fields.fields() {
                        let value = field.value().map(|value| self.lower_expr(value, scope));
                        if let Some(name) = field.name_token() {
                            self.file.object_fields.push(ObjectFieldInfo {
                                owner: expr_id,
                                name: normalize_object_field_name(name.text()),
                                range: name.text_range(),
                                value,
                            });
                        }
                    }
                }
            }
            Expr::If(if_expr) => {
                let condition = if_expr
                    .condition()
                    .map(|condition| self.lower_expr(condition, scope));
                let then_branch = if_expr
                    .then_branch()
                    .map(|then_branch| self.lower_block_expr(then_branch, scope));
                let else_branch = if_expr
                    .else_branch()
                    .and_then(|branch| branch.body())
                    .map(|else_branch| self.lower_expr(else_branch, scope));
                self.file.if_exprs.push(IfExprInfo {
                    owner: expr_id,
                    condition,
                    then_branch,
                    else_branch,
                });
                self.record_merge_point(MergePointKind::IfElse, if_expr.syntax().text_range());
            }
            Expr::Switch(switch_expr) => {
                let scrutinee = switch_expr
                    .scrutinee()
                    .map(|scrutinee| self.lower_expr(scrutinee, scope));
                let mut arms = Vec::new();
                if let Some(arm_list) = switch_expr.arm_list() {
                    for arm in arm_list.arms() {
                        arms.push(self.lower_switch_arm(arm, scope, expr_id));
                    }
                }
                self.file.switch_exprs.push(SwitchExprInfo {
                    owner: expr_id,
                    scrutinee,
                    arms,
                });
                self.record_merge_point(MergePointKind::Switch, switch_expr.syntax().text_range());
            }
            Expr::While(while_expr) => {
                if let Some(condition) = while_expr.condition() {
                    self.lower_expr(condition, scope);
                }
                if let Some(body) = while_expr.body() {
                    let loop_scope =
                        self.new_scope(ScopeKind::Loop, body.syntax().text_range(), Some(scope));
                    let body_id = self.new_body(
                        BodyKind::Block,
                        body.syntax().text_range(),
                        loop_scope,
                        None,
                    );
                    self.with_loop(loop_scope, |this| {
                        this.with_body(body_id, |this| this.lower_block_items(body, loop_scope))
                    });
                }
                self.record_merge_point(
                    MergePointKind::LoopIteration,
                    while_expr.syntax().text_range(),
                );
            }
            Expr::Loop(loop_expr) => {
                if let Some(body) = loop_expr.body() {
                    let loop_scope =
                        self.new_scope(ScopeKind::Loop, body.syntax().text_range(), Some(scope));
                    let body_id = self.new_body(
                        BodyKind::Block,
                        body.syntax().text_range(),
                        loop_scope,
                        None,
                    );
                    self.with_loop(loop_scope, |this| {
                        this.with_body(body_id, |this| this.lower_block_items(body, loop_scope))
                    });
                }
                self.record_merge_point(
                    MergePointKind::LoopIteration,
                    loop_expr.syntax().text_range(),
                );
            }
            Expr::For(for_expr) => {
                let iterable_expr = for_expr
                    .iterable()
                    .map(|iterable| self.lower_expr(iterable, scope));

                let loop_scope =
                    self.new_scope(ScopeKind::Loop, for_expr.syntax().text_range(), Some(scope));
                let mut binding_symbols = Vec::new();
                if let Some(bindings) = for_expr.bindings() {
                    for binding in bindings.names() {
                        binding_symbols.push(self.alloc_symbol(
                            binding.text().to_owned(),
                            SymbolKind::Variable,
                            binding.text_range(),
                            loop_scope,
                            None,
                        ));
                    }
                }
                let mut body_id = None;
                if let Some(body) = for_expr.body() {
                    let lowered_body = self.new_body(
                        BodyKind::Block,
                        body.syntax().text_range(),
                        loop_scope,
                        None,
                    );
                    self.with_loop(loop_scope, |this| {
                        this.with_body(lowered_body, |this| {
                            this.lower_block_items(body, loop_scope)
                        })
                    });
                    body_id = Some(lowered_body);
                }
                self.file.for_exprs.push(ForExprInfo {
                    owner: expr_id,
                    iterable: iterable_expr,
                    bindings: binding_symbols,
                    body: body_id,
                });
                self.record_merge_point(
                    MergePointKind::LoopIteration,
                    for_expr.syntax().text_range(),
                );
            }
            Expr::Do(do_expr) => {
                if let Some(body) = do_expr.body() {
                    let loop_scope =
                        self.new_scope(ScopeKind::Loop, body.syntax().text_range(), Some(scope));
                    let body_id = self.new_body(
                        BodyKind::Block,
                        body.syntax().text_range(),
                        loop_scope,
                        None,
                    );
                    self.with_loop(loop_scope, |this| {
                        this.with_body(body_id, |this| this.lower_block_items(body, loop_scope))
                    });
                }
                if let Some(condition) = do_expr.condition().and_then(|condition| condition.expr())
                {
                    self.lower_expr(condition, scope);
                }
                self.record_merge_point(
                    MergePointKind::LoopIteration,
                    do_expr.syntax().text_range(),
                );
            }
            Expr::Path(path) => {
                let mut base_expr = None;
                let mut rooted_global = false;
                if let Some(base) = path.base() {
                    if matches!(
                        &base,
                        Expr::Name(name)
                            if matches!(
                                name.token().map(|token| token.kind()),
                                Some(kind) if kind.token_kind() == Some(rhai_syntax::TokenKind::GlobalKw)
                            )
                    ) {
                        rooted_global = true;
                    } else {
                        base_expr = Some(self.lower_expr(base, scope));
                    }
                }
                let mut segment_references = Vec::new();
                for segment in path.segments() {
                    segment_references.push(self.alloc_reference(
                        segment.text().to_owned(),
                        ReferenceKind::PathSegment,
                        segment.text_range(),
                        scope,
                    ));
                }
                self.file.path_exprs.push(PathExprInfo {
                    owner: expr_id,
                    base: base_expr,
                    rooted_global,
                    segments: segment_references,
                });
            }
            Expr::Closure(closure) => {
                let closure_scope = self.new_scope(
                    ScopeKind::Closure,
                    closure.syntax().text_range(),
                    Some(scope),
                );
                let body_id = self.new_body(
                    BodyKind::Closure,
                    closure.syntax().text_range(),
                    closure_scope,
                    None,
                );
                let params = self.closure_param_bindings(&closure);
                self.lower_param_symbols(&params, None, closure_scope);
                if let Some(body) = closure.body() {
                    let body_expr =
                        self.with_body(body_id, |this| this.lower_expr(body, closure_scope));
                    self.file.bodies[body_id.0 as usize].tail_value = Some(body_expr);
                }
                self.file.closure_exprs.push(ClosureExprInfo {
                    owner: expr_id,
                    body: body_id,
                });
            }
            Expr::InterpolatedString(string) => {
                let interpolation_scope = self.new_scope(
                    ScopeKind::Interpolation,
                    string.syntax().text_range(),
                    Some(scope),
                );
                self.new_body(
                    BodyKind::Interpolation,
                    string.syntax().text_range(),
                    interpolation_scope,
                    None,
                );
                let body_id = BodyId((self.file.bodies.len() - 1) as u32);
                if let Some(parts) = string.part_list() {
                    for part in parts.parts() {
                        if let StringPart::Interpolation(part) = part
                            && let Some(body) = part.body()
                        {
                            self.with_body(body_id, |this| {
                                if let Some(items) = body.item_list() {
                                    for item in items.items() {
                                        this.lower_item(item, interpolation_scope);
                                    }
                                }
                            });
                        }
                    }
                }
            }
            Expr::Unary(unary) => {
                let operand = unary.expr().map(|expr| self.lower_expr(expr, scope));
                let operator_token = unary.operator_token();
                if let Some(token) = operator_token
                    && let Some(operator) = token.kind().token_kind().and_then(unary_operator)
                {
                    self.file.unary_exprs.push(UnaryExprInfo {
                        owner: expr_id,
                        operator,
                        operand,
                        operator_range: Some(token.text_range()),
                    });
                }
            }
            Expr::Binary(binary) => {
                let lhs = binary.lhs().map(|lhs| self.lower_expr(lhs, scope));
                let rhs = binary.rhs().map(|rhs| self.lower_expr(rhs, scope));
                let operator_token = binary.operator_token();
                if let Some(token) = operator_token
                    && let Some(operator) = token.kind().token_kind().and_then(binary_operator)
                {
                    self.file.binary_exprs.push(BinaryExprInfo {
                        owner: expr_id,
                        operator,
                        lhs,
                        rhs,
                        operator_range: Some(token.text_range()),
                    });
                }
            }
            Expr::Assign(assign) => {
                let assignment_start = self.file.references.len();
                let assignment_operator = assign
                    .operator_token()
                    .and_then(|token| token.kind().token_kind().and_then(assignment_operator));
                let lhs_syntax = assign.lhs();
                let lhs_expr = lhs_syntax.clone().map(|lhs| self.lower_expr(lhs, scope));
                let rhs_expr = assign.rhs().map(|rhs| self.lower_expr(rhs, scope));

                if let Some(operator) = assignment_operator {
                    self.file.assign_exprs.push(AssignExprInfo {
                        owner: expr_id,
                        operator,
                        lhs: lhs_expr,
                        rhs: rhs_expr,
                        operator_range: assign.operator_token().map(|token| token.text_range()),
                    });
                }

                let stored_value = match assignment_operator {
                    Some(AssignmentOperator::Assign) => rhs_expr,
                    Some(_) => Some(expr_id),
                    None => rhs_expr,
                };

                if let Some(lhs) = lhs_syntax
                    && let Some(value_expr) = stored_value
                {
                    if let Some(reference) = lhs_expr
                        .filter(|lhs_expr| self.file.expr(*lhs_expr).kind == ExprKind::Name)
                        .and_then(|lhs_expr| {
                            self.first_name_reference_from(
                                assignment_start,
                                self.file.expr(lhs_expr).range,
                            )
                        })
                    {
                        self.pending_value_flows.push(PendingValueFlow {
                            reference,
                            expr: value_expr,
                            kind: ValueFlowKind::Assignment,
                            range: assign.syntax().text_range(),
                        });
                    }

                    if let Some((receiver_reference, segments)) =
                        self.mutation_target_from_expr(assignment_start, &lhs)
                    {
                        self.pending_mutations.push(PendingMutation {
                            receiver_reference,
                            value: value_expr,
                            kind: PendingMutationKind::Path { segments },
                            range: assign.syntax().text_range(),
                        });
                    }
                }
            }
            Expr::Paren(paren) => {
                if let Some(expr) = paren.expr() {
                    self.lower_expr(expr, scope);
                }
            }
            Expr::Call(call) => {
                self.lower_call_expr(call, scope);
            }
            Expr::Index(index) => {
                let receiver = index
                    .receiver()
                    .map(|receiver| self.lower_expr(receiver, scope));
                let index = index.index().map(|expr| self.lower_expr(expr, scope));
                self.file.index_exprs.push(IndexExprInfo {
                    owner: expr_id,
                    receiver,
                    index,
                });
                if let Some((root_reference, segments)) = self.read_target_for_expr(expr_id) {
                    self.pending_reads.push(crate::lowering::ctx::PendingRead {
                        owner: expr_id,
                        root_reference,
                        segments,
                        range: self.file.expr(expr_id).range,
                    });
                }
            }
            Expr::Field(field) => {
                if let Some(receiver) = field.receiver() {
                    let receiver_expr = self.lower_expr(receiver, scope);
                    if let Some(name) = field.name_token() {
                        let field_reference = self.alloc_reference(
                            name.text().to_owned(),
                            ReferenceKind::Field,
                            name.text_range(),
                            scope,
                        );
                        self.file.member_accesses.push(MemberAccess {
                            owner: expr_id,
                            range: field.syntax().text_range(),
                            scope,
                            receiver: receiver_expr,
                            field_reference,
                        });
                        if let Some((root_reference, segments)) = self.read_target_for_expr(expr_id)
                        {
                            self.pending_reads.push(crate::lowering::ctx::PendingRead {
                                owner: expr_id,
                                root_reference,
                                segments,
                                range: self.file.expr(expr_id).range,
                            });
                        }
                    }
                }
            }
            Expr::Block(block) => {
                let body = self.lower_block_expr_with_owner(block, scope);
                self.file.block_exprs.push(BlockExprInfo {
                    owner: expr_id,
                    body,
                });
            }
        }

        expr_id
    }

    pub(crate) fn lower_block_expr(
        &mut self,
        block: BlockExpr,
        parent_scope: ScopeId,
    ) -> crate::ExprId {
        let expr_id = self.alloc_expr(ExprKind::Block, block.syntax().text_range(), parent_scope);
        let body = self.lower_block_expr_with_owner(block, parent_scope);
        self.file.block_exprs.push(BlockExprInfo {
            owner: expr_id,
            body,
        });
        expr_id
    }

    pub(crate) fn lower_block_expr_with_owner(
        &mut self,
        block: BlockExpr,
        parent_scope: ScopeId,
    ) -> BodyId {
        let block_scope = self.new_scope(
            ScopeKind::Block,
            block.syntax().text_range(),
            Some(parent_scope),
        );
        let body_id = self.new_body(
            BodyKind::Block,
            block.syntax().text_range(),
            block_scope,
            None,
        );
        self.with_body(body_id, |this| this.lower_block_items(block, block_scope));
        body_id
    }

    pub(crate) fn lower_switch_arm(
        &mut self,
        arm: SwitchArm,
        scope: ScopeId,
        owner: crate::ExprId,
    ) -> Option<crate::ExprId> {
        let arm_scope =
            self.new_scope(ScopeKind::SwitchArm, arm.syntax().text_range(), Some(scope));
        let mut lowered_patterns = Vec::new();
        let mut wildcard = false;
        if let Some(patterns) = arm.patterns() {
            wildcard = patterns.wildcard_token().is_some();
            lowered_patterns = self.lower_switch_patterns(patterns, arm_scope);
        }
        let value = arm.value().map(|value| self.lower_expr(value, arm_scope));
        self.file.switch_arms.push(crate::SwitchArmInfo {
            owner,
            scope: arm_scope,
            patterns: lowered_patterns,
            wildcard,
            value,
        });
        value
    }

    pub(crate) fn lower_switch_patterns(
        &mut self,
        patterns: SwitchPatternList,
        scope: ScopeId,
    ) -> Vec<crate::ExprId> {
        let mut lowered = Vec::new();
        for expr in patterns.exprs() {
            lowered.push(self.lower_expr(expr, scope));
        }
        lowered
    }

    pub(crate) fn item_may_fall_through(&self, item: &Item) -> bool {
        match item {
            Item::Fn(_) => true,
            Item::Stmt(stmt) => self.stmt_may_fall_through(stmt),
        }
    }

    pub(crate) fn stmt_may_fall_through(&self, stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Break(_) | Stmt::Continue(_) | Stmt::Return(_) | Stmt::Throw(_) => false,
            Stmt::Expr(stmt) => stmt
                .expr()
                .is_none_or(|expr| self.expr_may_fall_through(&expr)),
            Stmt::Let(stmt) => stmt
                .initializer()
                .is_none_or(|expr| self.expr_may_fall_through(&expr)),
            Stmt::Const(stmt) => stmt
                .value()
                .is_none_or(|expr| self.expr_may_fall_through(&expr)),
            Stmt::Try(stmt) => {
                let body_fallthrough = stmt
                    .body()
                    .is_none_or(|body| self.block_may_fall_through(&body));
                let catch_fallthrough = stmt
                    .catch_clause()
                    .and_then(|catch| catch.body())
                    .is_none_or(|body| self.block_may_fall_through(&body));
                body_fallthrough || catch_fallthrough
            }
            Stmt::Import(_) | Stmt::Export(_) => true,
        }
    }

    pub(crate) fn block_may_fall_through(&self, block: &BlockExpr) -> bool {
        let mut may_fall_through = true;
        if let Some(items) = block.item_list() {
            for item in items.items() {
                may_fall_through = self.item_may_fall_through(&item);
                if !may_fall_through {
                    break;
                }
            }
        }
        may_fall_through
    }

    pub(crate) fn expr_may_fall_through(&self, expr: &Expr) -> bool {
        match expr {
            Expr::If(if_expr) => {
                let then_fallthrough = if_expr
                    .then_branch()
                    .is_none_or(|block| self.block_may_fall_through(&block));
                let else_fallthrough = if_expr
                    .else_branch()
                    .and_then(|branch| branch.body())
                    .is_none_or(|expr| self.expr_may_fall_through(&expr));
                then_fallthrough || else_fallthrough
            }
            Expr::Switch(switch_expr) => {
                let mut saw_wildcard = false;
                let mut all_arms_terminate = true;
                if let Some(arm_list) = switch_expr.arm_list() {
                    for arm in arm_list.arms() {
                        if let Some(patterns) = arm.patterns()
                            && patterns.wildcard_token().is_some()
                        {
                            saw_wildcard = true;
                        }

                        let arm_fallthrough = arm
                            .value()
                            .is_none_or(|expr| self.expr_may_fall_through(&expr));
                        if arm_fallthrough {
                            all_arms_terminate = false;
                        }
                    }
                }

                !(saw_wildcard && all_arms_terminate)
            }
            Expr::Block(block) => self.block_may_fall_through(block),
            Expr::Paren(paren) => paren
                .expr()
                .is_none_or(|expr| self.expr_may_fall_through(&expr)),
            Expr::Do(_) | Expr::While(_) | Expr::Loop(_) | Expr::For(_) => true,
            _ => true,
        }
    }
}

fn expr_kind(expr: &Expr) -> ExprKind {
    match expr {
        Expr::Name(_) => ExprKind::Name,
        Expr::Literal(_) => ExprKind::Literal,
        Expr::Array(_) => ExprKind::Array,
        Expr::Object(_) => ExprKind::Object,
        Expr::If(_) => ExprKind::If,
        Expr::Switch(_) => ExprKind::Switch,
        Expr::While(_) => ExprKind::While,
        Expr::Loop(_) => ExprKind::Loop,
        Expr::For(_) => ExprKind::For,
        Expr::Do(_) => ExprKind::Do,
        Expr::Path(_) => ExprKind::Path,
        Expr::Closure(_) => ExprKind::Closure,
        Expr::InterpolatedString(_) => ExprKind::InterpolatedString,
        Expr::Unary(_) => ExprKind::Unary,
        Expr::Binary(_) => ExprKind::Binary,
        Expr::Assign(_) => ExprKind::Assign,
        Expr::Paren(_) => ExprKind::Paren,
        Expr::Call(_) => ExprKind::Call,
        Expr::Index(_) => ExprKind::Index,
        Expr::Field(_) => ExprKind::Field,
        Expr::Block(_) => ExprKind::Block,
        Expr::Error(_) => ExprKind::Error,
    }
}

fn normalize_object_field_name(text: &str) -> String {
    text.strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
        .unwrap_or(text)
        .to_owned()
}

fn literal_kind(kind: TokenKind) -> Option<LiteralKind> {
    match kind {
        TokenKind::Int => Some(LiteralKind::Int),
        TokenKind::Float => Some(LiteralKind::Float),
        TokenKind::String | TokenKind::RawString | TokenKind::BacktickString => {
            Some(LiteralKind::String)
        }
        TokenKind::Char => Some(LiteralKind::Char),
        TokenKind::TrueKw | TokenKind::FalseKw => Some(LiteralKind::Bool),
        _ => None,
    }
}

fn unary_operator(kind: TokenKind) -> Option<UnaryOperator> {
    match kind {
        TokenKind::Plus => Some(UnaryOperator::Plus),
        TokenKind::Minus => Some(UnaryOperator::Minus),
        TokenKind::Bang => Some(UnaryOperator::Not),
        _ => None,
    }
}

fn binary_operator(kind: TokenKind) -> Option<BinaryOperator> {
    match kind {
        TokenKind::PipePipe => Some(BinaryOperator::OrOr),
        TokenKind::Pipe => Some(BinaryOperator::Or),
        TokenKind::Caret => Some(BinaryOperator::Xor),
        TokenKind::AmpAmp => Some(BinaryOperator::AndAnd),
        TokenKind::Amp => Some(BinaryOperator::And),
        TokenKind::EqEq => Some(BinaryOperator::EqEq),
        TokenKind::BangEq => Some(BinaryOperator::NotEq),
        TokenKind::InKw => Some(BinaryOperator::In),
        TokenKind::Gt => Some(BinaryOperator::Gt),
        TokenKind::GtEq => Some(BinaryOperator::GtEq),
        TokenKind::Lt => Some(BinaryOperator::Lt),
        TokenKind::LtEq => Some(BinaryOperator::LtEq),
        TokenKind::QuestionQuestion => Some(BinaryOperator::NullCoalesce),
        TokenKind::Range => Some(BinaryOperator::Range),
        TokenKind::RangeEq => Some(BinaryOperator::RangeInclusive),
        TokenKind::Plus => Some(BinaryOperator::Add),
        TokenKind::Minus => Some(BinaryOperator::Subtract),
        TokenKind::Star => Some(BinaryOperator::Multiply),
        TokenKind::Slash => Some(BinaryOperator::Divide),
        TokenKind::Percent => Some(BinaryOperator::Remainder),
        TokenKind::StarStar => Some(BinaryOperator::Power),
        TokenKind::Shl => Some(BinaryOperator::ShiftLeft),
        TokenKind::Shr => Some(BinaryOperator::ShiftRight),
        _ => None,
    }
}

fn assignment_operator(kind: TokenKind) -> Option<AssignmentOperator> {
    match kind {
        TokenKind::Eq => Some(AssignmentOperator::Assign),
        TokenKind::QuestionQuestionEq => Some(AssignmentOperator::NullCoalesce),
        TokenKind::PlusEq => Some(AssignmentOperator::Add),
        TokenKind::MinusEq => Some(AssignmentOperator::Subtract),
        TokenKind::StarEq => Some(AssignmentOperator::Multiply),
        TokenKind::SlashEq => Some(AssignmentOperator::Divide),
        TokenKind::PercentEq => Some(AssignmentOperator::Remainder),
        TokenKind::StarStarEq => Some(AssignmentOperator::Power),
        TokenKind::ShlEq => Some(AssignmentOperator::ShiftLeft),
        TokenKind::ShrEq => Some(AssignmentOperator::ShiftRight),
        TokenKind::PipeEq => Some(AssignmentOperator::Or),
        TokenKind::CaretEq => Some(AssignmentOperator::Xor),
        TokenKind::AmpEq => Some(AssignmentOperator::And),
        _ => None,
    }
}
