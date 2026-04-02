use rhai_hir::SymbolKind;

use crate::completion::CompletionContext;
use crate::{CompletionItem, CompletionItemKind, CompletionItemSource};

pub(super) fn rank_completion_items(items: &mut [CompletionItem], context: &CompletionContext) {
    for item in items.iter_mut() {
        item.sort_text = completion_sort_text(item, context);
    }

    items.sort_by(|left, right| {
        left.sort_text
            .cmp(&right.sort_text)
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| {
                source_rank(left.source, context).cmp(&source_rank(right.source, context))
            })
    });
}

fn completion_sort_text(item: &CompletionItem, context: &CompletionContext) -> String {
    let relevance_rank = relevance_rank(item);
    let prefix_rank = prefix_match_rank(item.label.as_str(), context.prefix.as_str());
    let source_rank = source_rank(item.source, context);
    let kind_rank = kind_rank(item.kind);

    format!(
        "{relevance_rank}:{prefix_rank}:{source_rank}:{kind_rank}:{}",
        item.label.to_ascii_lowercase()
    )
}

fn relevance_rank(item: &CompletionItem) -> String {
    format!("{:010}", u32::MAX - item.relevance.score())
}

fn prefix_match_rank(label: &str, prefix: &str) -> u8 {
    if prefix.is_empty() {
        return 1;
    }

    let label_lower = label.to_ascii_lowercase();
    let prefix_lower = prefix.to_ascii_lowercase();

    if label_lower == prefix_lower {
        0
    } else if label_lower.starts_with(prefix_lower.as_str()) {
        1
    } else if label_lower.contains(prefix_lower.as_str()) {
        2
    } else {
        3
    }
}

pub(super) fn source_rank(source: CompletionItemSource, context: &CompletionContext) -> u8 {
    match (context.member_access, source) {
        (true, CompletionItemSource::Member) => 0,
        (true, CompletionItemSource::Builtin) => 1,
        (true, CompletionItemSource::Postfix) => 2,
        (true, CompletionItemSource::Visible) => 3,
        (true, CompletionItemSource::Module) => 4,
        (true, CompletionItemSource::Project) => 5,
        (true, CompletionItemSource::AutoImport) => 6,
        (false, CompletionItemSource::Visible) => 0,
        (false, CompletionItemSource::Module) => 1,
        (false, CompletionItemSource::Project) => 2,
        (false, CompletionItemSource::AutoImport) => 3,
        (false, CompletionItemSource::Builtin) => 4,
        (false, CompletionItemSource::Postfix) => 5,
        (false, CompletionItemSource::Member) => 6,
    }
}

fn kind_rank(kind: CompletionItemKind) -> u8 {
    match kind {
        CompletionItemKind::Member => 0,
        CompletionItemKind::Symbol(SymbolKind::Variable | SymbolKind::Parameter) => 0,
        CompletionItemKind::Symbol(SymbolKind::Constant) => 1,
        CompletionItemKind::Symbol(SymbolKind::Function) => 2,
        CompletionItemKind::Symbol(SymbolKind::ImportAlias | SymbolKind::ExportAlias) => 3,
        CompletionItemKind::Type => 4,
        CompletionItemKind::Snippet => 5,
        CompletionItemKind::Keyword => 6,
    }
}

#[cfg(test)]
mod tests {
    use crate::completion::CompletionContext;
    use crate::completion::ranking::{rank_completion_items, source_rank};
    use crate::types::{
        CompletionRelevance, CompletionRelevanceNameMatch, CompletionRelevancePostfixMatch,
    };
    use crate::{CompletionInsertFormat, CompletionItem, CompletionItemKind, CompletionItemSource};
    use rhai_hir::SymbolKind;
    use rhai_syntax::{TextRange, TextSize};

    fn test_context(member_access: bool) -> CompletionContext {
        CompletionContext {
            prefix: "sha".to_owned(),
            replace_range: TextRange::empty(TextSize::from(0)),
            query_offset: 0,
            member_access,
            module_path: None,
            postfix_completion: None,
            suppress_completion: false,
            doc_completion: None,
            next_char_is_open_paren: false,
        }
    }

    fn test_item(source: CompletionItemSource) -> CompletionItem {
        CompletionItem {
            label: "shared_helper".to_owned(),
            kind: CompletionItemKind::Symbol(SymbolKind::Function),
            source,
            relevance: CompletionRelevance::default(),
            origin: None,
            sort_text: String::new(),
            detail: None,
            docs: None,
            filter_text: None,
            text_edit: None,
            insert_format: CompletionInsertFormat::PlainText,
            file_id: None,
            exported: true,
            resolve_data: None,
        }
    }

    #[test]
    fn module_candidates_rank_ahead_of_project_candidates() {
        assert!(
            source_rank(CompletionItemSource::Module, &test_context(false))
                < source_rank(CompletionItemSource::Project, &test_context(false))
        );

        let context = test_context(false);
        let mut items = vec![
            test_item(CompletionItemSource::Project),
            test_item(CompletionItemSource::Module),
        ];

        rank_completion_items(&mut items, &context);

        assert_eq!(items[0].source, CompletionItemSource::Module);
        assert_eq!(items[1].source, CompletionItemSource::Project);
    }

    #[test]
    fn exact_postfix_matches_rank_ahead_of_other_candidates() {
        let context = CompletionContext {
            prefix: "switch".to_owned(),
            ..test_context(true)
        };
        let mut items = vec![
            CompletionItem {
                label: "switch".to_owned(),
                kind: CompletionItemKind::Snippet,
                source: CompletionItemSource::Postfix,
                relevance: CompletionRelevance {
                    postfix_match: Some(CompletionRelevancePostfixMatch::Exact),
                    ..CompletionRelevance::default()
                },
                origin: None,
                sort_text: String::new(),
                detail: None,
                docs: None,
                filter_text: Some("switch".to_owned()),
                text_edit: None,
                insert_format: CompletionInsertFormat::Snippet,
                file_id: None,
                exported: false,
                resolve_data: None,
            },
            CompletionItem {
                label: "substring".to_owned(),
                kind: CompletionItemKind::Member,
                source: CompletionItemSource::Member,
                relevance: CompletionRelevance::default(),
                origin: None,
                sort_text: String::new(),
                detail: None,
                docs: None,
                filter_text: None,
                text_edit: None,
                insert_format: CompletionInsertFormat::PlainText,
                file_id: None,
                exported: false,
                resolve_data: None,
            },
        ];

        rank_completion_items(&mut items, &context);

        assert_eq!(items[0].label, "switch");
        assert_eq!(items[0].source, CompletionItemSource::Postfix);
    }

    #[test]
    fn non_exact_postfix_matches_rank_after_normal_candidates() {
        let context = CompletionContext {
            prefix: "s".to_owned(),
            ..test_context(true)
        };
        let mut items = vec![
            CompletionItem {
                label: "switch".to_owned(),
                kind: CompletionItemKind::Snippet,
                source: CompletionItemSource::Postfix,
                relevance: CompletionRelevance {
                    postfix_match: Some(CompletionRelevancePostfixMatch::NonExact),
                    ..CompletionRelevance::default()
                },
                origin: None,
                sort_text: String::new(),
                detail: None,
                docs: None,
                filter_text: Some("switch".to_owned()),
                text_edit: None,
                insert_format: CompletionInsertFormat::Snippet,
                file_id: None,
                exported: false,
                resolve_data: None,
            },
            CompletionItem {
                label: "split".to_owned(),
                kind: CompletionItemKind::Member,
                source: CompletionItemSource::Member,
                relevance: CompletionRelevance::default(),
                origin: None,
                sort_text: String::new(),
                detail: None,
                docs: None,
                filter_text: None,
                text_edit: None,
                insert_format: CompletionInsertFormat::PlainText,
                file_id: None,
                exported: false,
                resolve_data: None,
            },
        ];

        rank_completion_items(&mut items, &context);

        assert_eq!(items[0].label, "split");
        assert_eq!(items[1].label, "switch");
    }

    #[test]
    fn name_match_relevance_prefers_prefix_over_contains() {
        let context = CompletionContext {
            prefix: "he".to_owned(),
            ..test_context(false)
        };
        let mut items = vec![
            CompletionItem {
                label: "the_helper".to_owned(),
                kind: CompletionItemKind::Symbol(SymbolKind::Function),
                source: CompletionItemSource::Visible,
                relevance: CompletionRelevance {
                    name_match: Some(CompletionRelevanceNameMatch::Contains),
                    ..CompletionRelevance::default()
                },
                origin: None,
                sort_text: String::new(),
                detail: None,
                docs: None,
                filter_text: None,
                text_edit: None,
                insert_format: CompletionInsertFormat::PlainText,
                file_id: None,
                exported: false,
                resolve_data: None,
            },
            CompletionItem {
                label: "helper".to_owned(),
                kind: CompletionItemKind::Symbol(SymbolKind::Function),
                source: CompletionItemSource::Visible,
                relevance: CompletionRelevance {
                    name_match: Some(CompletionRelevanceNameMatch::Prefix),
                    ..CompletionRelevance::default()
                },
                origin: None,
                sort_text: String::new(),
                detail: None,
                docs: None,
                filter_text: None,
                text_edit: None,
                insert_format: CompletionInsertFormat::PlainText,
                file_id: None,
                exported: false,
                resolve_data: None,
            },
        ];

        rank_completion_items(&mut items, &context);

        assert_eq!(items[0].label, "helper");
    }

    #[test]
    fn name_match_relevance_prefers_subsequence_over_contains() {
        let context = CompletionContext {
            prefix: "shr".to_owned(),
            ..test_context(false)
        };
        let mut items = vec![
            CompletionItem {
                label: "the_shared_helper".to_owned(),
                kind: CompletionItemKind::Symbol(SymbolKind::Function),
                source: CompletionItemSource::Visible,
                relevance: CompletionRelevance {
                    name_match: Some(CompletionRelevanceNameMatch::Contains),
                    ..CompletionRelevance::default()
                },
                origin: None,
                sort_text: String::new(),
                detail: None,
                docs: None,
                filter_text: None,
                text_edit: None,
                insert_format: CompletionInsertFormat::PlainText,
                file_id: None,
                exported: false,
                resolve_data: None,
            },
            CompletionItem {
                label: "shared_helper".to_owned(),
                kind: CompletionItemKind::Symbol(SymbolKind::Function),
                source: CompletionItemSource::Visible,
                relevance: CompletionRelevance {
                    name_match: Some(CompletionRelevanceNameMatch::Subsequence),
                    ..CompletionRelevance::default()
                },
                origin: None,
                sort_text: String::new(),
                detail: None,
                docs: None,
                filter_text: None,
                text_edit: None,
                insert_format: CompletionInsertFormat::PlainText,
                file_id: None,
                exported: false,
                resolve_data: None,
            },
        ];

        rank_completion_items(&mut items, &context);

        assert_eq!(items[0].label, "shared_helper");
    }

    #[test]
    fn non_import_candidates_rank_ahead_of_requires_import_candidates() {
        let context = CompletionContext {
            prefix: "helper".to_owned(),
            ..test_context(false)
        };
        let mut items = vec![
            CompletionItem {
                label: "helper".to_owned(),
                kind: CompletionItemKind::Symbol(SymbolKind::Function),
                source: CompletionItemSource::Project,
                relevance: CompletionRelevance {
                    requires_import: true,
                    name_match: Some(CompletionRelevanceNameMatch::Exact),
                    ..CompletionRelevance::default()
                },
                origin: None,
                sort_text: String::new(),
                detail: None,
                docs: None,
                filter_text: None,
                text_edit: None,
                insert_format: CompletionInsertFormat::PlainText,
                file_id: None,
                exported: false,
                resolve_data: None,
            },
            CompletionItem {
                label: "helper".to_owned(),
                kind: CompletionItemKind::Symbol(SymbolKind::Function),
                source: CompletionItemSource::Visible,
                relevance: CompletionRelevance {
                    name_match: Some(CompletionRelevanceNameMatch::Exact),
                    ..CompletionRelevance::default()
                },
                origin: None,
                sort_text: String::new(),
                detail: None,
                docs: None,
                filter_text: None,
                text_edit: None,
                insert_format: CompletionInsertFormat::PlainText,
                file_id: None,
                exported: false,
                resolve_data: None,
            },
        ];

        rank_completion_items(&mut items, &context);

        assert_eq!(items[0].source, CompletionItemSource::Visible);
        assert!(items[1].relevance.requires_import);
    }

    #[test]
    fn lower_import_cost_ranks_ahead_when_both_candidates_require_imports() {
        let context = CompletionContext {
            prefix: "helper".to_owned(),
            ..test_context(false)
        };
        let mut items = vec![
            CompletionItem {
                label: "helper".to_owned(),
                kind: CompletionItemKind::Symbol(SymbolKind::Function),
                source: CompletionItemSource::Project,
                relevance: CompletionRelevance {
                    requires_import: true,
                    import_cost: Some(3),
                    name_match: Some(CompletionRelevanceNameMatch::Exact),
                    ..CompletionRelevance::default()
                },
                origin: None,
                sort_text: String::new(),
                detail: None,
                docs: None,
                filter_text: None,
                text_edit: None,
                insert_format: CompletionInsertFormat::PlainText,
                file_id: None,
                exported: false,
                resolve_data: None,
            },
            CompletionItem {
                label: "helper".to_owned(),
                kind: CompletionItemKind::Symbol(SymbolKind::Function),
                source: CompletionItemSource::Project,
                relevance: CompletionRelevance {
                    requires_import: true,
                    import_cost: Some(1),
                    name_match: Some(CompletionRelevanceNameMatch::Exact),
                    ..CompletionRelevance::default()
                },
                origin: None,
                sort_text: String::new(),
                detail: None,
                docs: None,
                filter_text: None,
                text_edit: None,
                insert_format: CompletionInsertFormat::PlainText,
                file_id: None,
                exported: false,
                resolve_data: None,
            },
        ];

        rank_completion_items(&mut items, &context);

        assert_eq!(items[0].relevance.import_cost, Some(1));
        assert_eq!(items[1].relevance.import_cost, Some(3));
    }
}
