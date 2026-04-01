use rhai_db::DatabaseSnapshot;
use rhai_hir::{FileHir, ImportDirective};
use rhai_syntax::TextRange;
use rhai_vfs::FileId;

use crate::{Assist, AssistId, AssistKind, FileTextEdit, SourceChange, TextEdit};

const REMOVE_UNUSED_IMPORT_ASSIST_ID: AssistId = AssistId::new("import.remove_unused");
const ORGANIZE_IMPORTS_ASSIST_ID: AssistId = AssistId::new("import.organize");

pub(crate) fn remove_unused_imports(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
) -> Option<SourceChange> {
    let hir = snapshot.hir(file_id)?;
    let file_text = snapshot.file_text(file_id)?;

    let mut edits = hir
        .imports
        .iter()
        .filter(|import| is_unused_import(hir.as_ref(), import))
        .map(|import| remove_import_edit(import.range, file_text.as_ref()))
        .collect::<Vec<_>>();

    if edits.is_empty() {
        return None;
    }

    edits.sort_by(|left, right| {
        left.range
            .start()
            .cmp(&right.range.start())
            .then_with(|| left.range.end().cmp(&right.range.end()))
    });

    Some(SourceChange::new(vec![FileTextEdit::new(file_id, edits)]))
}

pub(crate) fn organize_imports(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
) -> Option<SourceChange> {
    let hir = snapshot.hir(file_id)?;
    let file_text = snapshot.file_text(file_id)?;
    let block = import_block(hir.as_ref(), file_text.as_ref())?;

    let mut rendered_imports = block
        .imports
        .iter()
        .map(|import| render_import(hir.as_ref(), import))
        .collect::<Vec<_>>();
    rendered_imports.sort();
    rendered_imports.dedup();

    let new_text = rendered_imports.join("\n");
    let old_text =
        file_text.get(usize::from(block.range.start())..usize::from(block.range.end()))?;
    if old_text == new_text {
        return None;
    }

    Some(SourceChange::from_text_edit(
        file_id,
        TextEdit::replace(block.range, new_text),
    ))
}

pub(crate) fn unused_import_assists_for_diagnostic(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    diagnostic_range: TextRange,
) -> Vec<Assist> {
    let Some(hir) = snapshot.hir(file_id) else {
        return Vec::new();
    };
    let Some(file_text) = snapshot.file_text(file_id) else {
        return Vec::new();
    };

    hir.imports
        .iter()
        .filter(|import| is_unused_import(hir.as_ref(), import))
        .filter_map(|import| {
            let alias = import.alias?;
            let alias_range = hir.symbol(alias).range;
            (alias_range == diagnostic_range).then_some(Assist {
                id: REMOVE_UNUSED_IMPORT_ASSIST_ID,
                kind: AssistKind::QuickFix,
                group: Some("Import".to_owned()),
                label: "Remove unused import".to_owned(),
                target: diagnostic_range,
                source_change: SourceChange::from_text_edit(
                    file_id,
                    remove_import_edit(import.range, file_text.as_ref()),
                ),
            })
        })
        .collect()
}

pub(crate) fn import_source_assists(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    range: TextRange,
) -> Vec<Assist> {
    let Some(hir) = snapshot.hir(file_id) else {
        return Vec::new();
    };
    let Some(file_text) = snapshot.file_text(file_id) else {
        return Vec::new();
    };
    let Some(block) = import_block(hir.as_ref(), file_text.as_ref()) else {
        return Vec::new();
    };

    if !ranges_intersect_or_touch(range, block.range) {
        return Vec::new();
    }

    let mut assists = Vec::new();
    if let Some(source_change) = remove_unused_imports(snapshot, file_id) {
        assists.push(Assist {
            id: REMOVE_UNUSED_IMPORT_ASSIST_ID,
            kind: AssistKind::Source,
            group: Some("Import".to_owned()),
            label: "Remove unused imports".to_owned(),
            target: block.range,
            source_change,
        });
    }
    if let Some(source_change) = organize_imports(snapshot, file_id) {
        assists.push(Assist {
            id: ORGANIZE_IMPORTS_ASSIST_ID,
            kind: AssistKind::Source,
            group: Some("Import".to_owned()),
            label: "Organize imports".to_owned(),
            target: block.range,
            source_change,
        });
    }

    assists
}

pub(crate) fn ranges_intersect_or_touch(selection: TextRange, target: TextRange) -> bool {
    if selection.start() == selection.end() {
        let point = selection.start();
        return target.start() <= point && point <= target.end();
    }

    selection.start() < target.end() && target.start() < selection.end()
}

pub(crate) fn remove_import_edit(import_range: TextRange, file_text: &str) -> TextEdit {
    let bytes = file_text.as_bytes();
    let mut start = usize::from(import_range.start());
    let mut end = usize::from(import_range.end());

    if end < bytes.len() {
        if bytes[end] == b'\r' {
            end += 1;
            if end < bytes.len() && bytes[end] == b'\n' {
                end += 1;
            }
        } else if bytes[end] == b'\n' {
            end += 1;
        }
    } else {
        while start > 0 && matches!(bytes[start - 1], b'\r' | b'\n') {
            start -= 1;
            if start > 0 && bytes[start - 1] == b'\r' && bytes[start] == b'\n' {
                start -= 1;
            }
        }
    }

    TextEdit::replace(
        TextRange::new((start as u32).into(), (end as u32).into()),
        "",
    )
}

fn is_unused_import(hir: &FileHir, import: &ImportDirective) -> bool {
    let Some(alias) = import.alias else {
        return false;
    };
    let symbol = hir.symbol(alias);
    symbol.references.is_empty() && !symbol.name.starts_with('_')
}

fn render_import(hir: &FileHir, import: &ImportDirective) -> String {
    let module = import
        .module_text
        .as_deref()
        .or_else(|| {
            import
                .module_reference
                .map(|reference_id| hir.reference(reference_id).name.as_str())
        })
        .unwrap_or("");

    match import.alias {
        Some(alias) => format!("import {module} as {};", hir.symbol(alias).name),
        None => format!("import {module};"),
    }
}

struct ImportBlock<'a> {
    range: TextRange,
    imports: Vec<&'a ImportDirective>,
}

fn import_block<'a>(hir: &'a FileHir, file_text: &str) -> Option<ImportBlock<'a>> {
    if hir.imports.is_empty() {
        return None;
    }

    let mut imports = hir.imports.iter().collect::<Vec<_>>();
    imports.sort_by(|left, right| left.range.start().cmp(&right.range.start()));

    for pair in imports.windows(2) {
        let [left, right] = pair else {
            continue;
        };
        let between =
            file_text.get(usize::from(left.range.end())..usize::from(right.range.start()))?;
        if between.chars().any(|ch| !ch.is_whitespace()) {
            return None;
        }
    }

    let first = imports.first()?;
    let last = imports.last()?;
    Some(ImportBlock {
        range: TextRange::new(first.range.start(), last.range.end()),
        imports,
    })
}
