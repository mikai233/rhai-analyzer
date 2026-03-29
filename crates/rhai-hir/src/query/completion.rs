use std::collections::{BTreeMap, HashSet};

use rhai_syntax::TextSize;

use crate::model::{
    CompletionSymbol, FileHir, MemberAccess, MemberCompletion, MemberCompletionSource,
    WorkspaceSymbol,
};

impl FileHir {
    pub fn completion_symbols_at(&self, offset: TextSize) -> Vec<CompletionSymbol> {
        self.visible_symbols_at(offset)
            .into_iter()
            .map(|symbol_id| {
                let symbol = self.symbol(symbol_id);
                CompletionSymbol {
                    symbol: symbol_id,
                    name: symbol.name.clone(),
                    kind: symbol.kind,
                    range: symbol.range,
                    docs: symbol.docs,
                    annotation: symbol.annotation.clone(),
                }
            })
            .collect()
    }

    pub fn project_completion_symbols_at(
        &self,
        offset: TextSize,
        workspace: &[WorkspaceSymbol],
    ) -> Vec<WorkspaceSymbol> {
        let local_names = self
            .visible_symbols_at(offset)
            .into_iter()
            .map(|symbol| self.symbol(symbol).name.clone())
            .collect::<HashSet<_>>();

        workspace
            .iter()
            .filter(|symbol| !local_names.contains(symbol.name.as_str()))
            .cloned()
            .collect()
    }

    pub fn member_completion_at(&self, offset: TextSize) -> Vec<MemberCompletion> {
        let Some(access) = self.member_access_at_offset(offset) else {
            return Vec::new();
        };

        self.member_completions_for_expr(access.receiver)
    }

    pub(crate) fn member_access_at_offset(&self, offset: TextSize) -> Option<&MemberAccess> {
        self.member_accesses
            .iter()
            .filter(|access| {
                access.range.contains(offset)
                    || self
                        .reference(access.field_reference)
                        .range
                        .contains(offset)
            })
            .min_by_key(|access| access.range.len())
    }

    pub(crate) fn member_completions_for_expr(&self, expr: crate::ExprId) -> Vec<MemberCompletion> {
        let mut members = BTreeMap::<String, MemberCompletion>::new();

        for field in self
            .object_fields
            .iter()
            .filter(|field| field.owner == expr)
        {
            members
                .entry(field.name.clone())
                .or_insert(MemberCompletion {
                    name: field.name.clone(),
                    annotation: field
                        .value
                        .and_then(|value| self.object_field_annotation_from_expr(value)),
                    range: Some(field.range),
                    source: MemberCompletionSource::ObjectLiteralField,
                });
        }

        if let Some(symbol) = self.symbol_for_expr(expr) {
            for field in self.documented_fields(symbol) {
                members
                    .entry(field.name.clone())
                    .or_insert(MemberCompletion {
                        name: field.name,
                        annotation: Some(field.annotation),
                        range: None,
                        source: MemberCompletionSource::DocumentedField,
                    });
            }

            for flow in self.value_flows_into(symbol) {
                for field in self
                    .object_fields
                    .iter()
                    .filter(|field| field.owner == flow.expr)
                {
                    members
                        .entry(field.name.clone())
                        .or_insert(MemberCompletion {
                            name: field.name.clone(),
                            annotation: field
                                .value
                                .and_then(|value| self.object_field_annotation_from_expr(value)),
                            range: Some(field.range),
                            source: MemberCompletionSource::ObjectLiteralField,
                        });
                }
            }
        }

        members.into_values().collect()
    }
}
