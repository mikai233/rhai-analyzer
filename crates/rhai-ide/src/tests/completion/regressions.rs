use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::{AnalysisHost, FilePosition};

#[test]
fn completions_do_not_panic_when_offset_lands_inside_multibyte_punctuation() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "import \"demo\" as dd;\n\nlet q = 1.0 + 2。;\n\ndd::test();\nfn v() {}\n",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = analysis.db.file_text(file_id).expect("expected text");
    let punctuation_offset =
        u32::try_from(text.find('。').expect("expected unicode punctuation") + 1).expect("offset");

    let _completions = analysis.completions(FilePosition {
        file_id,
        offset: punctuation_offset,
    });
}
