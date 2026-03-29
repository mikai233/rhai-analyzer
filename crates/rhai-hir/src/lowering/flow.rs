use crate::docs::DocBlockId;
use crate::lowering::ctx::LoweringContext;
use crate::model::{
    Body, BodyId, BodyKind, ControlFlowEvent, ControlFlowKind, ControlFlowMergePoint, ExprId,
    MergePointKind, SymbolId, SymbolValueFlow, ValueFlowKind,
};
use crate::ty::TypeRef;
use rhai_syntax::TextRange;

impl<'a> LoweringContext<'a> {
    pub(crate) fn push_value_flow(
        &mut self,
        symbol: SymbolId,
        expr: ExprId,
        kind: ValueFlowKind,
        range: TextRange,
    ) {
        self.file.value_flows.push(SymbolValueFlow {
            symbol,
            expr,
            kind,
            range,
        });
    }

    pub(crate) fn with_body<T>(&mut self, body: BodyId, f: impl FnOnce(&mut Self) -> T) -> T {
        self.body_stack.push(body);
        let result = f(self);
        let popped = self.body_stack.pop();
        debug_assert_eq!(popped, Some(body));
        result
    }

    pub(crate) fn with_loop<T>(
        &mut self,
        loop_scope: crate::ScopeId,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.loop_stack.push(loop_scope);
        let result = f(self);
        let popped = self.loop_stack.pop();
        debug_assert_eq!(popped, Some(loop_scope));
        result
    }

    pub(crate) fn record_control_flow(
        &mut self,
        kind: ControlFlowKind,
        range: TextRange,
        value_range: Option<TextRange>,
    ) {
        let target_loop = match kind {
            ControlFlowKind::Break | ControlFlowKind::Continue => self.loop_stack.last().copied(),
            ControlFlowKind::Return | ControlFlowKind::Throw => None,
        };
        for &body_id in self.body_stack.iter().rev() {
            self.file.bodies[body_id.0 as usize]
                .control_flow
                .push(ControlFlowEvent {
                    kind,
                    range,
                    value_range,
                    target_loop,
                });

            if self.is_body_flow_boundary(body_id) {
                break;
            }
        }
    }

    pub(crate) fn record_body_value(&mut self, kind: ControlFlowKind, expr: ExprId) {
        for &body_id in self.body_stack.iter().rev() {
            let body = &mut self.file.bodies[body_id.0 as usize];
            match kind {
                ControlFlowKind::Return => body.return_values.push(expr),
                ControlFlowKind::Throw => body.throw_values.push(expr),
                ControlFlowKind::Break | ControlFlowKind::Continue => {}
            }

            if self.is_body_flow_boundary(body_id) {
                break;
            }
        }
    }

    pub(crate) fn record_merge_point(&mut self, kind: MergePointKind, range: TextRange) {
        for &body_id in self.body_stack.iter().rev() {
            self.file.bodies[body_id.0 as usize]
                .merge_points
                .push(ControlFlowMergePoint { kind, range });

            if self.is_body_flow_boundary(body_id) {
                break;
            }
        }
    }

    pub(crate) fn is_body_flow_boundary(&self, body: BodyId) -> bool {
        matches!(
            self.file.bodies[body.0 as usize].kind,
            BodyKind::Function | BodyKind::Closure | BodyKind::Interpolation
        )
    }

    pub(crate) fn current_body_mut(&mut self) -> Option<&mut Body> {
        let body = *self.body_stack.last()?;
        Some(&mut self.file.bodies[body.0 as usize])
    }

    pub(crate) fn lower_function_signature(
        &self,
        docs: Option<DocBlockId>,
        params: &[(String, TextRange)],
    ) -> Option<TypeRef> {
        let names = params
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        self.function_annotation_from_docs(docs, &names)
    }
}
