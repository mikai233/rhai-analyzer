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

mod completion;
mod diagnostics;
mod hover;
mod imports;
mod rename;
mod signature_help;
