use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::{DocumentVersion, FileId};

use crate::tests::assert_no_syntax_diagnostics;
use crate::{Analysis, AnalysisHost, FilePosition};

pub(crate) mod constructors;
pub(crate) mod introspection;
pub(crate) mod methods;
pub(crate) mod parsing;

pub(crate) fn load_analysis(source: &str) -> (Analysis, FileId, String) {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        source,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");

    (analysis, file_id, text.to_string())
}

pub(crate) fn signature_help_at(
    analysis: &Analysis,
    file_id: FileId,
    offset: u32,
) -> crate::SignatureHelp {
    analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected signature help")
}
