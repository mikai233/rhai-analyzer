use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, FilePosition, HoverSignatureSource};

fn assert_structured_builtin_docs(docs: &str, topic: &str) {
    assert!(!docs.trim().is_empty());
    assert!(docs.contains("## Usage"));
    assert!(docs.contains("## Examples"));
    assert!(docs.contains("## Official Rhai Reference"));
    assert!(docs.contains(topic));
}

#[test]
fn hover_supports_array_indexing_topics() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let items = [10, 20, 30];
                let value = items[1];
            }
        "#,
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
    let offset = u32::try_from(text.rfind('[').expect("expected index operator")).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected array index hover");

    assert_eq!(hover.signature, "array[index] -> any");
    assert_eq!(hover.source, HoverSignatureSource::Structural);
    let docs = hover.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "array");
}

#[test]
fn hover_supports_bit_field_indexing_topics() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let flags = 10;
                let value = flags[1];
            }
        "#,
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
    let offset = u32::try_from(text.find('[').expect("expected index operator")).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected bit-field index hover");

    assert_eq!(hover.signature, "int[index] -> bool");
    assert_eq!(hover.source, HoverSignatureSource::Structural);
    let docs = hover.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "bit-field");
}

#[test]
fn hover_supports_map_property_topics() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let user = #{ name: "Ada" };
                let value = user.name;
            }
        "#,
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
    let offset =
        u32::try_from(text.find('.').expect("expected property operator")).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected property access hover");

    assert_eq!(hover.signature, "map.field -> any | ()");
    assert_eq!(hover.source, HoverSignatureSource::Structural);
    let docs = hover.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "property");
}
