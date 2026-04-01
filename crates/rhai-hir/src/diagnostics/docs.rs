use std::collections::{HashMap, HashSet};

use rhai_syntax::TextRange;

use crate::docs::DocTag;
use crate::model::{
    FileHir, SemanticDiagnostic, SemanticDiagnosticCode, SemanticDiagnosticKind, SymbolId,
    SymbolKind,
};
use crate::ty::TypeRef;

impl FileHir {
    pub(crate) fn doc_type_consistency_diagnostics(&self) -> Vec<SemanticDiagnostic> {
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
                        code: SemanticDiagnosticCode::DuplicateDocParamTag {
                            name: name.to_owned(),
                        },
                        range: docs.range,
                        message: format!("duplicate `@param` tag for `{name}`"),
                        related_range: Some(symbol.range),
                    });
                }
            }

            if return_tag_count > 1 {
                diagnostics.push(SemanticDiagnostic {
                    kind: SemanticDiagnosticKind::InconsistentDocType,
                    code: SemanticDiagnosticCode::DuplicateDocReturnTag,
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
                            code: SemanticDiagnosticCode::FunctionDocTagsOnNonFunction {
                                symbol: symbol.name.clone(),
                            },
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
        docs_range: TextRange,
    ) -> Vec<SemanticDiagnostic> {
        let mut diagnostics = Vec::new();
        let function = self.symbol(symbol);

        if let Some(annotation) = &function.annotation
            && !matches!(annotation, TypeRef::Function(_))
        {
            diagnostics.push(SemanticDiagnostic {
                kind: SemanticDiagnosticKind::InconsistentDocType,
                code: SemanticDiagnosticCode::FunctionHasNonFunctionTypeAnnotation {
                    function: function.name.clone(),
                },
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
                        code: SemanticDiagnosticCode::DocParamDoesNotMatchFunction {
                            name: name.clone(),
                            function: function.name.clone(),
                        },
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
}
