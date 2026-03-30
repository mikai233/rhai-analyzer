use std::collections::BTreeMap;

use crate::support::convert::text_size;
use crate::{DocumentHighlight, DocumentHighlightKind, FilePosition};
use rhai_db::DatabaseSnapshot;
use rhai_hir::ReferenceKind as HirReferenceKind;

pub(crate) fn document_highlights(
    snapshot: &DatabaseSnapshot,
    position: FilePosition,
) -> Vec<DocumentHighlight> {
    if let Some(hir) = snapshot.hir(position.file_id)
        && let Some(references) = hir.find_references(text_size(position.offset))
    {
        return dedupe_document_highlights(
            std::iter::once(DocumentHighlight {
                range: references.declaration.full_range,
                kind: DocumentHighlightKind::Write,
            })
            .chain(
                references
                    .references
                    .into_iter()
                    .map(|reference| DocumentHighlight {
                        range: reference.range,
                        kind: hir_reference_highlight_kind(reference.kind),
                    }),
            )
            .collect(),
        );
    }

    snapshot
        .find_references(position.file_id, text_size(position.offset))
        .map(|references| {
            dedupe_document_highlights(
                references
                    .references
                    .into_iter()
                    .filter(|reference| reference.file_id == position.file_id)
                    .map(|reference| DocumentHighlight {
                        range: reference.range,
                        kind: match reference.kind {
                            rhai_db::ProjectReferenceKind::Definition => {
                                DocumentHighlightKind::Write
                            }
                            rhai_db::ProjectReferenceKind::Reference => DocumentHighlightKind::Read,
                            rhai_db::ProjectReferenceKind::LinkedImport => {
                                DocumentHighlightKind::Text
                            }
                        },
                    })
                    .collect(),
            )
        })
        .unwrap_or_default()
}

fn hir_reference_highlight_kind(kind: HirReferenceKind) -> DocumentHighlightKind {
    match kind {
        HirReferenceKind::Name | HirReferenceKind::This | HirReferenceKind::Field => {
            DocumentHighlightKind::Read
        }
        HirReferenceKind::PathSegment => DocumentHighlightKind::Text,
    }
}

fn dedupe_document_highlights(highlights: Vec<DocumentHighlight>) -> Vec<DocumentHighlight> {
    let mut by_key = BTreeMap::<(u32, u32, u8), DocumentHighlight>::new();

    for highlight in highlights {
        by_key.insert(
            (
                u32::from(highlight.range.start()),
                u32::from(highlight.range.end()),
                highlight_kind_rank(highlight.kind),
            ),
            highlight,
        );
    }

    by_key.into_values().collect()
}

fn highlight_kind_rank(kind: DocumentHighlightKind) -> u8 {
    match kind {
        DocumentHighlightKind::Write => 0,
        DocumentHighlightKind::Read => 1,
        DocumentHighlightKind::Text => 2,
    }
}
