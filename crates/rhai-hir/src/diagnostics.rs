use std::collections::{HashMap, HashSet};

use crate::{
    DocTag, FileHir, ReferenceId, ReferenceKind, SemanticDiagnostic, SemanticDiagnosticKind,
    Symbol, SymbolId, SymbolKind, TypeRef,
};

impl FileHir {
    pub fn diagnostics(&self) -> Vec<SemanticDiagnostic> {
        let mut diagnostics = self.unresolved_import_export_diagnostics();
        diagnostics.extend(self.invalid_import_module_type_diagnostics());
        diagnostics.extend(self.invalid_export_target_diagnostics());
        diagnostics.extend(self.unresolved_name_diagnostics());
        diagnostics.extend(self.duplicate_definition_diagnostics());
        diagnostics.extend(self.doc_type_consistency_diagnostics());
        diagnostics.extend(self.unused_symbol_diagnostics());
        diagnostics
    }

    fn unresolved_name_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        let handled = self.import_export_reference_ids();
        self.references
            .iter()
            .enumerate()
            .filter(|reference| {
                let (index, reference) = reference;
                matches!(reference.kind, ReferenceKind::Name | ReferenceKind::This)
                    && reference.target.is_none()
                    && reference.name != "this"
                    && !handled.contains(&ReferenceId(*index as u32))
            })
            .map(|(_, reference)| SemanticDiagnostic {
                kind: SemanticDiagnosticKind::UnresolvedName,
                range: reference.range,
                message: format!("unresolved name `{}`", reference.name),
                related_range: None,
            })
            .collect()
    }

    fn unresolved_import_export_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        let mut diagnostics = Vec::new();

        for import in &self.imports {
            if let Some(reference_id) = import.module_reference {
                let reference = self.reference(reference_id);
                if reference.target.is_none() {
                    diagnostics.push(SemanticDiagnostic {
                        kind: SemanticDiagnosticKind::UnresolvedImport,
                        range: reference.range,
                        message: format!("unresolved import module `{}`", reference.name),
                        related_range: Some(import.range),
                    });
                }
            }
        }

        for export in &self.exports {
            if export.target_symbol.is_some() {
                continue;
            }
            if let Some(reference_id) = export.target_reference {
                let reference = self.reference(reference_id);
                if reference.target.is_none() {
                    diagnostics.push(SemanticDiagnostic {
                        kind: SemanticDiagnosticKind::UnresolvedExport,
                        range: reference.range,
                        message: format!("unresolved export target `{}`", reference.name),
                        related_range: Some(export.range),
                    });
                }
            }
        }

        diagnostics
    }

    fn invalid_export_target_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        self.exports
            .iter()
            .filter_map(|export| {
                let reference_id = export.target_reference?;
                let symbol_id = self.definition_of(reference_id)?;
                let symbol = self.symbol(symbol_id);
                (matches!(
                    symbol.kind,
                    SymbolKind::Function
                        | SymbolKind::ImportAlias
                        | SymbolKind::ExportAlias
                        | SymbolKind::Parameter
                ) || !matches!(symbol.kind, SymbolKind::Variable | SymbolKind::Constant)
                    || self.scope(symbol.scope).kind != crate::ScopeKind::File)
                    .then(|| SemanticDiagnostic {
                        kind: SemanticDiagnosticKind::InvalidExportTarget,
                        range: self.reference(reference_id).range,
                        message: format!(
                            "export target `{}` must refer to a global variable or constant",
                            self.reference(reference_id).name
                        ),
                        related_range: Some(export.range),
                    })
            })
            .collect()
    }

    fn invalid_import_module_type_diagnostics(&self) -> Vec<SemanticDiagnostic> {
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

    fn duplicate_definition_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        self.symbols
            .iter()
            .filter_map(|symbol| {
                let previous = symbol.duplicate_of?;
                Some(SemanticDiagnostic {
                    kind: SemanticDiagnosticKind::DuplicateDefinition,
                    range: symbol.range,
                    message: format!("duplicate definition of `{}`", symbol.name),
                    related_range: Some(self.symbol(previous).range),
                })
            })
            .collect()
    }

    fn doc_type_consistency_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        let mut diagnostics = Vec::new();

        for (index, symbol) in self.symbols.iter().enumerate() {
            let Some(doc_id) = symbol.docs else {
                continue;
            };
            let symbol_id = SymbolId(index as u32);
            let docs = self.doc_block(doc_id);

            let mut param_tag_counts = HashMap::<&str, usize>::new();
            let mut return_tag_count = 0usize;

            for tag in &docs.tags {
                match tag {
                    DocTag::Param { name, .. } => {
                        *param_tag_counts.entry(name.as_str()).or_default() += 1;
                    }
                    DocTag::Return(_) => {
                        return_tag_count += 1;
                    }
                    _ => {}
                }
            }

            for (name, count) in param_tag_counts {
                if count > 1 {
                    diagnostics.push(SemanticDiagnostic {
                        kind: SemanticDiagnosticKind::InconsistentDocType,
                        range: docs.range,
                        message: format!("duplicate `@param` tag for `{name}`"),
                        related_range: Some(symbol.range),
                    });
                }
            }

            if return_tag_count > 1 {
                diagnostics.push(SemanticDiagnostic {
                    kind: SemanticDiagnosticKind::InconsistentDocType,
                    range: docs.range,
                    message: "duplicate `@return` tags".to_owned(),
                    related_range: Some(symbol.range),
                });
            }

            match symbol.kind {
                SymbolKind::Function => {
                    diagnostics.extend(self.function_doc_type_diagnostics(symbol_id, docs.range));
                }
                _ => {
                    if docs
                        .tags
                        .iter()
                        .any(|tag| matches!(tag, DocTag::Param { .. } | DocTag::Return(_)))
                    {
                        diagnostics.push(SemanticDiagnostic {
                            kind: SemanticDiagnosticKind::InconsistentDocType,
                            range: docs.range,
                            message: format!(
                                "function doc tags cannot be attached to `{}`",
                                symbol.name
                            ),
                            related_range: Some(symbol.range),
                        });
                    }
                }
            }
        }

        diagnostics
    }

    fn function_doc_type_diagnostics(
        &self,
        symbol: SymbolId,
        docs_range: rhai_syntax::TextRange,
    ) -> Vec<SemanticDiagnostic> {
        let mut diagnostics = Vec::new();
        let function = self.symbol(symbol);

        if let Some(annotation) = &function.annotation
            && !matches!(annotation, TypeRef::Function(_))
        {
            diagnostics.push(SemanticDiagnostic {
                kind: SemanticDiagnosticKind::InconsistentDocType,
                range: docs_range,
                message: format!(
                    "function `{}` has a non-function type annotation",
                    function.name
                ),
                related_range: Some(function.range),
            });
        }

        let Some(body_id) = self.body_of(symbol) else {
            return diagnostics;
        };
        let params = self
            .scope(self.body(body_id).scope)
            .symbols
            .iter()
            .copied()
            .filter(|symbol_id| self.symbol(*symbol_id).kind == SymbolKind::Parameter)
            .map(|symbol_id| self.symbol(symbol_id).name.as_str())
            .collect::<HashSet<_>>();

        if let Some(doc_id) = function.docs {
            for tag in &self.doc_block(doc_id).tags {
                if let DocTag::Param { name, .. } = tag
                    && !params.contains(name.as_str())
                {
                    diagnostics.push(SemanticDiagnostic {
                        kind: SemanticDiagnosticKind::InconsistentDocType,
                        range: docs_range,
                        message: format!(
                            "doc tag `@param {name}` does not match any parameter of `{}`",
                            function.name
                        ),
                        related_range: Some(function.range),
                    });
                }
            }
        }

        diagnostics
    }

    fn unused_symbol_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        self.symbols
            .iter()
            .filter(|symbol| self.is_unused_symbol_candidate(symbol))
            .filter(|symbol| symbol.references.is_empty())
            .map(|symbol| SemanticDiagnostic {
                kind: SemanticDiagnosticKind::UnusedSymbol,
                range: symbol.range,
                message: format!("unused symbol `{}`", symbol.name),
                related_range: None,
            })
            .collect()
    }

    fn is_unused_symbol_candidate(&self, symbol: &Symbol) -> bool {
        matches!(
            symbol.kind,
            SymbolKind::Variable
                | SymbolKind::Parameter
                | SymbolKind::Constant
                | SymbolKind::ImportAlias
        ) && !symbol.name.starts_with('_')
    }

    fn import_export_reference_ids(&self) -> HashSet<ReferenceId> {
        let mut references = HashSet::new();
        for import in &self.imports {
            if let Some(reference) = import.module_reference {
                references.insert(reference);
            }
        }
        for export in &self.exports {
            if export.target_symbol.is_none()
                && let Some(reference) = export.target_reference
            {
                references.insert(reference);
            }
        }
        references
    }

    fn static_import_module_type(
        &self,
        expr: crate::ExprId,
        visited_symbols: &mut Vec<SymbolId>,
    ) -> Option<StaticImportModuleType> {
        match self.expr(expr).kind {
            crate::ExprKind::Literal => self.literal(expr).map(|literal| match literal.kind {
                crate::LiteralKind::String => StaticImportModuleType::String,
                crate::LiteralKind::Int => StaticImportModuleType::NonString("int"),
                crate::LiteralKind::Float => StaticImportModuleType::NonString("float"),
                crate::LiteralKind::Char => StaticImportModuleType::NonString("char"),
                crate::LiteralKind::Bool => StaticImportModuleType::NonString("bool"),
            }),
            crate::ExprKind::InterpolatedString => Some(StaticImportModuleType::String),
            crate::ExprKind::Array => Some(StaticImportModuleType::NonString("array")),
            crate::ExprKind::Object => Some(StaticImportModuleType::NonString("object map")),
            crate::ExprKind::Closure => Some(StaticImportModuleType::NonString("closure")),
            crate::ExprKind::Name => self
                .reference_at(self.expr(expr).range)
                .and_then(|reference| self.definition_of(reference))
                .and_then(|symbol| {
                    self.static_import_module_type_for_symbol(symbol, visited_symbols)
                }),
            crate::ExprKind::Paren => largest_inner_expr(self, expr)
                .and_then(|inner| self.static_import_module_type(inner, visited_symbols)),
            crate::ExprKind::Block => self.block_expr(expr).and_then(|block| {
                self.static_import_module_type_for_body(block.body, visited_symbols)
            }),
            crate::ExprKind::If => self.if_expr(expr).and_then(|if_expr| {
                self.static_import_module_type_for_if(if_expr, visited_symbols)
            }),
            crate::ExprKind::Binary => self.binary_expr(expr).and_then(|binary| {
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
                        .filter(|flow| flow.kind == crate::ValueFlowKind::Initializer)
                        .collect::<Vec<_>>();
                    let has_assignments = self
                        .value_flows_into(symbol)
                        .any(|flow| flow.kind == crate::ValueFlowKind::Assignment);
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
        body: crate::BodyId,
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
        if_expr: &crate::IfExprInfo,
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
        binary: &crate::BinaryExprInfo,
        visited_symbols: &mut Vec<SymbolId>,
    ) -> Option<StaticImportModuleType> {
        (binary.operator == crate::BinaryOperator::Add).then_some(())?;
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

fn largest_inner_expr(hir: &FileHir, expr: crate::ExprId) -> Option<crate::ExprId> {
    let range = hir.expr(expr).range;
    hir.exprs
        .iter()
        .enumerate()
        .filter(|(index, node)| {
            let candidate = crate::ExprId(*index as u32);
            candidate != expr
                && node.range.start() >= range.start()
                && node.range.end() <= range.end()
                && node.range != range
        })
        .max_by_key(|(_, node)| node.range.len())
        .map(|(index, _)| crate::ExprId(index as u32))
}
