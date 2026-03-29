use crate::model::{
    BinaryExprInfo, BinaryOperator, BodyId, ExprId, ExprKind, FileHir, IfExprInfo,
    SemanticDiagnostic, SemanticDiagnosticKind, SymbolId, SymbolKind, ValueFlowKind,
};
use crate::ty::TypeRef;

impl FileHir {
    pub(crate) fn invalid_import_module_type_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        self.imports
            .iter()
            .filter_map(|import| {
                let module_range = import.module_range?;
                let module_expr = self.expr_at(module_range)?;
                match self.static_import_module_type(module_expr, &mut Vec::new())? {
                    StaticImportModuleType::String => None,
                    StaticImportModuleType::NonString(found) => Some(SemanticDiagnostic {
                        kind: SemanticDiagnosticKind::InvalidImportModuleType,
                        range: module_range,
                        message: format!(
                            "import module expression `{}` must evaluate to string, found {found}",
                            import.module_text.as_deref().unwrap_or("<expr>")
                        ),
                        related_range: Some(import.range),
                    }),
                }
            })
            .collect()
    }

    fn static_import_module_type(
        &self,
        expr: ExprId,
        visited_symbols: &mut Vec<SymbolId>,
    ) -> Option<StaticImportModuleType> {
        match self.expr(expr).kind {
            ExprKind::Literal => self.literal(expr).map(|literal| match literal.kind {
                crate::LiteralKind::String => StaticImportModuleType::String,
                crate::LiteralKind::Int => StaticImportModuleType::NonString("int"),
                crate::LiteralKind::Float => StaticImportModuleType::NonString("float"),
                crate::LiteralKind::Char => StaticImportModuleType::NonString("char"),
                crate::LiteralKind::Bool => StaticImportModuleType::NonString("bool"),
            }),
            ExprKind::InterpolatedString => Some(StaticImportModuleType::String),
            ExprKind::Array => Some(StaticImportModuleType::NonString("array")),
            ExprKind::Object => Some(StaticImportModuleType::NonString("object map")),
            ExprKind::Closure => Some(StaticImportModuleType::NonString("closure")),
            ExprKind::Name => self
                .reference_at(self.expr(expr).range)
                .and_then(|reference| self.definition_of(reference))
                .and_then(|symbol| {
                    self.static_import_module_type_for_symbol(symbol, visited_symbols)
                }),
            ExprKind::Paren => largest_inner_expr(self, expr)
                .and_then(|inner| self.static_import_module_type(inner, visited_symbols)),
            ExprKind::Block => self.block_expr(expr).and_then(|block| {
                self.static_import_module_type_for_body(block.body, visited_symbols)
            }),
            ExprKind::If => self.if_expr(expr).and_then(|if_expr| {
                self.static_import_module_type_for_if(if_expr, visited_symbols)
            }),
            ExprKind::Binary => self.binary_expr(expr).and_then(|binary| {
                self.static_import_module_type_for_binary(binary, visited_symbols)
            }),
            _ => None,
        }
    }

    fn static_import_module_type_for_symbol(
        &self,
        symbol: SymbolId,
        visited_symbols: &mut Vec<SymbolId>,
    ) -> Option<StaticImportModuleType> {
        if visited_symbols.contains(&symbol) {
            return None;
        }
        visited_symbols.push(symbol);

        let result = match self.symbol(symbol).kind {
            SymbolKind::Function => Some(StaticImportModuleType::NonString("function")),
            SymbolKind::ImportAlias | SymbolKind::ExportAlias => {
                Some(StaticImportModuleType::NonString("module"))
            }
            SymbolKind::Variable | SymbolKind::Constant | SymbolKind::Parameter => self
                .declared_symbol_type(symbol)
                .and_then(static_import_module_type_from_type_ref)
                .or_else(|| {
                    let flows = self
                        .value_flows_into(symbol)
                        .filter(|flow| flow.kind == ValueFlowKind::Initializer)
                        .collect::<Vec<_>>();
                    let has_assignments = self
                        .value_flows_into(symbol)
                        .any(|flow| flow.kind == ValueFlowKind::Assignment);
                    (!has_assignments && flows.len() == 1)
                        .then(|| self.static_import_module_type(flows[0].expr, visited_symbols))
                        .flatten()
                }),
        };

        visited_symbols.pop();
        result
    }

    fn static_import_module_type_for_body(
        &self,
        body: BodyId,
        visited_symbols: &mut Vec<SymbolId>,
    ) -> Option<StaticImportModuleType> {
        if let Some(tail) = self.body_tail_value(body) {
            return self.static_import_module_type(tail, visited_symbols);
        }

        self.body_may_fall_through(body)
            .then_some(StaticImportModuleType::NonString("unit"))
    }

    fn static_import_module_type_for_if(
        &self,
        if_expr: &IfExprInfo,
        visited_symbols: &mut Vec<SymbolId>,
    ) -> Option<StaticImportModuleType> {
        let then_branch = if_expr
            .then_branch
            .and_then(|expr| self.static_import_module_type(expr, visited_symbols))?;
        let else_branch = if let Some(expr) = if_expr.else_branch {
            self.static_import_module_type(expr, visited_symbols)?
        } else {
            StaticImportModuleType::NonString("unit")
        };

        match (then_branch, else_branch) {
            (StaticImportModuleType::String, StaticImportModuleType::String) => {
                Some(StaticImportModuleType::String)
            }
            (StaticImportModuleType::NonString(left), StaticImportModuleType::NonString(right))
                if left == right =>
            {
                Some(StaticImportModuleType::NonString(left))
            }
            _ => None,
        }
    }

    fn static_import_module_type_for_binary(
        &self,
        binary: &BinaryExprInfo,
        visited_symbols: &mut Vec<SymbolId>,
    ) -> Option<StaticImportModuleType> {
        (binary.operator == BinaryOperator::Add).then_some(())?;
        let lhs = binary
            .lhs
            .and_then(|expr| self.static_import_module_type(expr, visited_symbols))?;
        let rhs = binary
            .rhs
            .and_then(|expr| self.static_import_module_type(expr, visited_symbols))?;

        match (lhs, rhs) {
            (StaticImportModuleType::String, StaticImportModuleType::String) => {
                Some(StaticImportModuleType::String)
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StaticImportModuleType {
    String,
    NonString(&'static str),
}

fn static_import_module_type_from_type_ref(ty: &TypeRef) -> Option<StaticImportModuleType> {
    match ty {
        TypeRef::String => Some(StaticImportModuleType::String),
        TypeRef::Bool => Some(StaticImportModuleType::NonString("bool")),
        TypeRef::Int => Some(StaticImportModuleType::NonString("int")),
        TypeRef::Float => Some(StaticImportModuleType::NonString("float")),
        TypeRef::Decimal => Some(StaticImportModuleType::NonString("decimal")),
        TypeRef::Char => Some(StaticImportModuleType::NonString("char")),
        TypeRef::Blob => Some(StaticImportModuleType::NonString("blob")),
        TypeRef::Timestamp => Some(StaticImportModuleType::NonString("timestamp")),
        TypeRef::FnPtr => Some(StaticImportModuleType::NonString("function pointer")),
        TypeRef::Unit => Some(StaticImportModuleType::NonString("unit")),
        TypeRef::Range => Some(StaticImportModuleType::NonString("range")),
        TypeRef::RangeInclusive => Some(StaticImportModuleType::NonString("inclusive range")),
        TypeRef::Object(_) => Some(StaticImportModuleType::NonString("object map")),
        TypeRef::Array(_) => Some(StaticImportModuleType::NonString("array")),
        TypeRef::Map(_, _) => Some(StaticImportModuleType::NonString("map")),
        TypeRef::Function(_) => Some(StaticImportModuleType::NonString("function")),
        TypeRef::Unknown
        | TypeRef::Any
        | TypeRef::Never
        | TypeRef::Dynamic
        | TypeRef::Named(_)
        | TypeRef::Applied { .. }
        | TypeRef::Nullable(_)
        | TypeRef::Union(_) => None,
    }
}

fn largest_inner_expr(hir: &FileHir, expr: ExprId) -> Option<ExprId> {
    let range = hir.expr(expr).range;
    hir.exprs
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
