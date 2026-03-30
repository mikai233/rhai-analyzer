use rhai_vfs::FileId;

pub(crate) fn assert_no_syntax_diagnostics(analysis: &crate::Analysis, file_id: FileId) {
    let diagnostics = analysis.db.syntax_diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected no syntax diagnostics for {:?}, got {:?}",
        file_id,
        diagnostics
    );
}

mod call_hierarchy;
mod completion;
mod diagnostics;
mod folding_ranges;
mod formatting;
mod highlights;
mod hover;
mod imports;
mod inlay_hints;
mod rename;
mod semantic_tokens;
mod signature_help;
