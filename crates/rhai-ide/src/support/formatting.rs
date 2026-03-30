use rhai_db::DatabaseSnapshot;
use rhai_fmt::{FormatOptions, format_range as format_range_text, format_text};
use rhai_syntax::{TextRange, TextSize};
use rhai_vfs::FileId;

use crate::{SourceChange, TextEdit};

pub(crate) fn format_document(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
) -> Option<SourceChange> {
    let text = snapshot.file_text(file_id)?;
    let formatted = format_text(text.as_ref(), &FormatOptions::default());
    if !formatted.changed {
        return None;
    }

    let full_range = TextRange::new(TextSize::from(0), TextSize::from(text.len() as u32));
    Some(SourceChange::from_text_edit(
        file_id,
        TextEdit::replace(full_range, formatted.text),
    ))
}

pub(crate) fn format_range(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    range: TextRange,
) -> Option<SourceChange> {
    let text = snapshot.file_text(file_id)?;
    let formatted = format_range_text(text.as_ref(), range, &FormatOptions::default())?;
    if !formatted.changed {
        return None;
    }

    Some(SourceChange::from_text_edit(
        file_id,
        TextEdit::replace(formatted.range, formatted.text),
    ))
}
