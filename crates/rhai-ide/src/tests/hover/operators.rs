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
fn hover_supports_membership_operator_topics() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let ok = 2 in [1, 2, 3];
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
    let offset = u32::try_from(text.find("in").expect("expected in operator")).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected in operator hover");

    assert_eq!(hover.signature, "value in array -> bool");
    assert_eq!(hover.source, HoverSignatureSource::Structural);
    let docs = hover.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "contains");
}

#[test]
fn hover_supports_range_operator_topics() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let span = 1..=3;
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
        u32::try_from(text.find("..=").expect("expected range operator") + 1).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected range operator hover");

    assert_eq!(hover.signature, "start ..= end -> range=");
    assert_eq!(hover.source, HoverSignatureSource::Structural);
    let docs = hover.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "inclusive range");
}

#[test]
fn hover_supports_numeric_addition_operator_topics() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let total = 20 + 22;
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
        u32::try_from(text.find('+').expect("expected addition operator")).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected addition operator hover");

    assert_eq!(hover.signature, "number + number -> number");
    assert_eq!(hover.source, HoverSignatureSource::Structural);
    let docs = hover.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "numeric");
}

#[test]
fn hover_supports_assignment_operator_topics() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let total = 10;
                total += 5;
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
        u32::try_from(text.find("+=").expect("expected assignment operator") + 1).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected assignment operator hover");

    assert_eq!(hover.signature, "number += number");
    assert_eq!(hover.source, HoverSignatureSource::Structural);
    let docs = hover.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "numeric");
}

#[test]
fn hover_supports_unary_minus_operator_topics() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let delta = -42;
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
    let offset = u32::try_from(text.find('-').expect("expected unary minus")).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected unary minus hover");

    assert_eq!(hover.signature, "-number -> number");
    assert_eq!(hover.source, HoverSignatureSource::Structural);
    let docs = hover.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "numeric");
}

#[test]
fn hover_supports_logical_not_operator_topics() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let ready = !false;
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
    let offset = u32::try_from(text.find('!').expect("expected logical not")).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected logical not hover");

    assert_eq!(hover.signature, "!bool -> bool");
    assert_eq!(hover.source, HoverSignatureSource::Structural);
    let docs = hover.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "boolean");
}
