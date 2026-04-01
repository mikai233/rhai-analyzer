use rhai_syntax::*;

use crate::ImportSortOrder;
use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;

impl Formatter<'_> {
    pub(crate) fn format_root(&self, root: Root) -> Doc {
        let items = root
            .item_list()
            .map(|items| items.items().collect::<Vec<_>>())
            .unwrap_or_default();
        let entries = self.root_entries(&items);
        let entry_ranges = entries
            .iter()
            .map(|entry| (entry.start, entry.end))
            .collect::<Vec<_>>();
        let owned = self.owned_range_sequence_trivia(
            u32::from(root.syntax().text_range().start()) as usize,
            u32::from(root.syntax().text_range().end()) as usize,
            &entry_ranges,
        );
        let mut parts = self.format_sequence_body_doc(
            entries.iter().map(|entry| entry.doc.clone()).collect(),
            &owned,
            |index| {
                if index > 0 {
                    root_item_separator_min_newlines(entries[index - 1].kind, entries[index].kind)
                } else {
                    1
                }
            },
        );

        self.append_sequence_trailing_doc(&mut parts, &owned.trailing, !items.is_empty(), 1);

        if parts.is_empty() {
            Doc::nil()
        } else {
            parts.push(Doc::hard_line());
            Doc::concat(parts)
        }
    }

    pub(crate) fn format_root_item_list_body_doc(&self, item_list: RootItemList) -> Doc {
        let items = item_list.items().collect::<Vec<_>>();
        let entries = self.root_entries(&items);
        let entry_ranges = entries
            .iter()
            .map(|entry| (entry.start, entry.end))
            .collect::<Vec<_>>();
        let owned = self.owned_range_sequence_trivia(
            u32::from(item_list.syntax().text_range().start()) as usize,
            u32::from(item_list.syntax().text_range().end()) as usize,
            &entry_ranges,
        );
        let parts = self.format_sequence_body_doc(
            entries.iter().map(|entry| entry.doc.clone()).collect(),
            &owned,
            |index| {
                if index > 0 {
                    root_item_separator_min_newlines(entries[index - 1].kind, entries[index].kind)
                } else {
                    1
                }
            },
        );

        Doc::concat(parts)
    }

    fn root_entries(&self, items: &[Item]) -> Vec<RootEntry> {
        let mut entries = Vec::new();
        let mut index = 0;

        while index < items.len() {
            let item = items[index].clone();
            if matches!(item, Item::Stmt(Stmt::Import(_)))
                && let Some((entry, next_index)) = self.sorted_import_entry(items, index)
            {
                entries.push(entry);
                index = next_index;
                continue;
            }

            entries.push(RootEntry {
                start: u32::from(item.syntax().text_range().start()) as usize,
                end: u32::from(item.syntax().text_range().end()) as usize,
                kind: top_level_item_kind(item.clone()),
                doc: self.format_item(item.clone(), 0),
            });
            index += 1;
        }

        entries
    }

    fn sorted_import_entry(
        &self,
        items: &[Item],
        start_index: usize,
    ) -> Option<(RootEntry, usize)> {
        if self.options.import_sort_order == ImportSortOrder::Preserve {
            return None;
        }

        let mut end_index = start_index;
        while end_index < items.len() && matches!(items[end_index], Item::Stmt(Stmt::Import(_))) {
            if end_index > start_index
                && self.import_boundary_starts_new_group(
                    items[end_index - 1].clone(),
                    items[end_index].clone(),
                )
            {
                break;
            }
            end_index += 1;
        }

        if end_index - start_index < 2 {
            return None;
        }

        let import_items = &items[start_index..end_index];
        if !self.can_reorder_import_run(import_items) {
            return None;
        }

        let mut rendered_imports = import_items
            .iter()
            .map(|item| self.render_fragment(&self.format_item(item.clone(), 0), 0))
            .collect::<Vec<_>>();
        rendered_imports.sort();

        let mut parts = Vec::new();
        for (index, import) in rendered_imports.into_iter().enumerate() {
            if index > 0 {
                parts.push(Doc::hard_line());
            }
            parts.push(Doc::text(import));
        }

        Some((
            RootEntry {
                start: u32::from(import_items[0].syntax().text_range().start()) as usize,
                end: u32::from(
                    import_items[import_items.len() - 1]
                        .syntax()
                        .text_range()
                        .end(),
                ) as usize,
                kind: TopLevelItemKind::Import,
                doc: Doc::concat(parts),
            },
            end_index,
        ))
    }

    fn can_reorder_import_run(&self, items: &[Item]) -> bool {
        !items.iter().any(|item| self.is_skipped(item.syntax()))
            && items.windows(2).all(|pair| {
                let [left, right] = pair else {
                    return true;
                };
                self.is_whitespace_only_between_nodes(left.syntax(), right.syntax())
            })
    }

    fn import_boundary_starts_new_group(&self, left: Item, right: Item) -> bool {
        self.has_blank_line_between_nodes(left.syntax(), right.syntax())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TopLevelItemKind {
    Import,
    Export,
    Function,
    Other,
}

fn root_item_separator_min_newlines(
    previous_kind: TopLevelItemKind,
    current_kind: TopLevelItemKind,
) -> usize {
    if previous_kind == TopLevelItemKind::Function || current_kind == TopLevelItemKind::Function {
        return 2;
    }

    if previous_kind != current_kind
        && matches!(
            previous_kind,
            TopLevelItemKind::Import | TopLevelItemKind::Export
        )
    {
        return 2;
    }

    if previous_kind != current_kind
        && matches!(
            current_kind,
            TopLevelItemKind::Import | TopLevelItemKind::Export
        )
    {
        return 2;
    }

    1
}

fn top_level_item_kind(item: Item) -> TopLevelItemKind {
    match item {
        Item::Fn(_) => TopLevelItemKind::Function,
        Item::Stmt(Stmt::Import(_)) => TopLevelItemKind::Import,
        Item::Stmt(Stmt::Export(_)) => TopLevelItemKind::Export,
        Item::Stmt(_) => TopLevelItemKind::Other,
    }
}

#[derive(Debug, Clone)]

struct RootEntry {
    start: usize,
    end: usize,
    kind: TopLevelItemKind,
    doc: Doc,
}
