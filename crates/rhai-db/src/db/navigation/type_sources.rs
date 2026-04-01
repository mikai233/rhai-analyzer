use crate::db::DatabaseSnapshot;
use crate::infer::{field_value_exprs_from_expr, largest_inner_expr};
use crate::types::LocatedNavigationTarget;
use rhai_hir::{FileHir, SymbolId, SymbolKind};
use rhai_vfs::FileId;
use std::collections::BTreeSet;

impl DatabaseSnapshot {
    pub(crate) fn type_source_targets_for_symbol(
        &self,
        file_id: FileId,
        hir: &FileHir,
        symbol: SymbolId,
    ) -> Vec<LocatedNavigationTarget> {
        let exprs = type_source_exprs_from_symbol(hir, symbol);
        let mut targets =
            self.navigation_targets_for_type_source_exprs(file_id, hir, exprs, Some(symbol));
        if targets.is_empty()
            && let Some(target) = documented_type_target(file_id, hir, symbol)
        {
            targets.push(target);
        }
        targets
    }

    pub(crate) fn type_source_targets_for_expr(
        &self,
        file_id: FileId,
        hir: &FileHir,
        expr: rhai_hir::ExprId,
    ) -> Vec<LocatedNavigationTarget> {
        let exprs = type_source_exprs_from_expr(hir, expr);
        self.navigation_targets_for_type_source_exprs(file_id, hir, exprs, None)
    }

    pub(crate) fn navigation_targets_for_type_source_exprs(
        &self,
        file_id: FileId,
        hir: &FileHir,
        exprs: Vec<rhai_hir::ExprId>,
        owner_symbol_hint: Option<SymbolId>,
    ) -> Vec<LocatedNavigationTarget> {
        let mut targets = exprs
            .into_iter()
            .map(|expr| {
                let range = hir.expr(expr).range;
                let owner_symbol = owner_symbol_hint.or_else(|| owner_symbol_for_expr(hir, expr));
                LocatedNavigationTarget {
                    file_id,
                    target: rhai_hir::NavigationTarget {
                        symbol: owner_symbol.unwrap_or(SymbolId(0)),
                        kind: owner_symbol
                            .map(|symbol| hir.symbol(symbol).kind)
                            .unwrap_or(SymbolKind::Variable),
                        full_range: range,
                        focus_range: range,
                    },
                }
            })
            .collect::<Vec<_>>();

        targets.sort_by(|left, right| {
            left.file_id.0.cmp(&right.file_id.0).then_with(|| {
                left.target
                    .full_range
                    .start()
                    .cmp(&right.target.full_range.start())
            })
        });
        targets
            .dedup_by(|left, right| left.file_id == right.file_id && left.target == right.target);
        targets
    }
}

fn type_source_exprs_from_expr(hir: &FileHir, expr: rhai_hir::ExprId) -> Vec<rhai_hir::ExprId> {
    let mut visited_exprs = BTreeSet::<u32>::new();
    let mut visited_symbols = BTreeSet::<u32>::new();
    let mut exprs = Vec::<rhai_hir::ExprId>::new();
    collect_type_source_exprs_from_expr(
        hir,
        expr,
        &mut visited_exprs,
        &mut visited_symbols,
        &mut exprs,
    );
    exprs.sort_by_key(|expr| expr.0);
    exprs.dedup_by_key(|expr| expr.0);
    exprs
}

fn type_source_exprs_from_symbol(hir: &FileHir, symbol: SymbolId) -> Vec<rhai_hir::ExprId> {
    let mut visited_exprs = BTreeSet::<u32>::new();
    let mut visited_symbols = BTreeSet::<u32>::new();
    let mut exprs = Vec::<rhai_hir::ExprId>::new();
    collect_type_source_exprs_from_symbol(
        hir,
        symbol,
        &mut visited_exprs,
        &mut visited_symbols,
        &mut exprs,
    );
    exprs.sort_by_key(|expr| expr.0);
    exprs.dedup_by_key(|expr| expr.0);
    exprs
}

fn collect_type_source_exprs_from_expr(
    hir: &FileHir,
    expr: rhai_hir::ExprId,
    visited_exprs: &mut BTreeSet<u32>,
    visited_symbols: &mut BTreeSet<u32>,
    out: &mut Vec<rhai_hir::ExprId>,
) {
    if !visited_exprs.insert(expr.0) {
        return;
    }

    match hir.expr(expr).kind {
        rhai_hir::ExprKind::Object | rhai_hir::ExprKind::Array | rhai_hir::ExprKind::Closure => {
            out.push(expr);
        }
        rhai_hir::ExprKind::Paren => {
            if let Some(inner) = largest_inner_expr(hir, expr) {
                collect_type_source_exprs_from_expr(
                    hir,
                    inner,
                    visited_exprs,
                    visited_symbols,
                    out,
                );
            }
        }
        rhai_hir::ExprKind::Name => {
            if let Some(symbol) = symbol_for_expr(hir, expr) {
                collect_type_source_exprs_from_symbol(
                    hir,
                    symbol,
                    visited_exprs,
                    visited_symbols,
                    out,
                );
            }
        }
        rhai_hir::ExprKind::Field => {
            if let Some(access) = hir.member_access(expr) {
                let field_name = hir.reference(access.field_reference).name.as_str();
                for value_expr in field_value_exprs_from_expr(hir, access.receiver, field_name) {
                    collect_type_source_exprs_from_expr(
                        hir,
                        value_expr,
                        visited_exprs,
                        visited_symbols,
                        out,
                    );
                }
            }
        }
        rhai_hir::ExprKind::Block => {
            if let Some(block) = hir.block_expr(expr)
                && let Some(tail) = hir.body_tail_value(block.body)
            {
                collect_type_source_exprs_from_expr(hir, tail, visited_exprs, visited_symbols, out);
            }
        }
        rhai_hir::ExprKind::If => {
            if let Some(if_expr) = hir.if_expr(expr) {
                for branch in [if_expr.then_branch, if_expr.else_branch]
                    .into_iter()
                    .flatten()
                {
                    collect_type_source_exprs_from_expr(
                        hir,
                        branch,
                        visited_exprs,
                        visited_symbols,
                        out,
                    );
                }
            }
        }
        rhai_hir::ExprKind::Switch => {
            if let Some(switch_expr) = hir.switch_expr(expr) {
                for arm in switch_expr.arms.iter().flatten().copied() {
                    collect_type_source_exprs_from_expr(
                        hir,
                        arm,
                        visited_exprs,
                        visited_symbols,
                        out,
                    );
                }
            }
        }
        _ => {}
    }
}

fn collect_type_source_exprs_from_symbol(
    hir: &FileHir,
    symbol: SymbolId,
    visited_exprs: &mut BTreeSet<u32>,
    visited_symbols: &mut BTreeSet<u32>,
    out: &mut Vec<rhai_hir::ExprId>,
) {
    if !visited_symbols.insert(symbol.0) {
        return;
    }

    for flow in hir.value_flows_into(symbol) {
        collect_type_source_exprs_from_expr(hir, flow.expr, visited_exprs, visited_symbols, out);
    }

    for mutation in hir.symbol_mutations_into(symbol) {
        match &mutation.kind {
            rhai_hir::SymbolMutationKind::Path { segments } if segments.is_empty() => {
                collect_type_source_exprs_from_expr(
                    hir,
                    mutation.value,
                    visited_exprs,
                    visited_symbols,
                    out,
                );
            }
            _ => {}
        }
    }
}

fn symbol_for_expr(hir: &FileHir, expr: rhai_hir::ExprId) -> Option<SymbolId> {
    (hir.expr(expr).kind == rhai_hir::ExprKind::Name)
        .then(|| hir.reference_at(hir.expr(expr).range))
        .flatten()
        .and_then(|reference| hir.definition_of(reference))
}

fn owner_symbol_for_expr(hir: &FileHir, expr: rhai_hir::ExprId) -> Option<SymbolId> {
    for (index, _) in hir.symbols.iter().enumerate() {
        let symbol = SymbolId(index as u32);
        if hir.value_flows_into(symbol).any(|flow| flow.expr == expr) {
            return Some(symbol);
        }
        if hir
            .symbol_mutations_into(symbol)
            .any(|mutation| mutation.value == expr)
        {
            return Some(symbol);
        }
    }
    None
}

fn documented_type_target(
    file_id: FileId,
    hir: &FileHir,
    symbol: SymbolId,
) -> Option<LocatedNavigationTarget> {
    let symbol_data = hir.symbol(symbol);
    symbol_data.annotation.as_ref()?;
    let doc_id = symbol_data.docs?;
    let docs = hir.doc_block(doc_id);
    Some(LocatedNavigationTarget {
        file_id,
        target: rhai_hir::NavigationTarget {
            symbol,
            kind: symbol_data.kind,
            full_range: docs.range,
            focus_range: docs.range,
        },
    })
}
