use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::{DocumentVersion, normalize_path};

use crate::{AnalysisHost, FilePosition};

#[test]
fn renaming_static_import_module_reference_renames_file_and_updates_imports() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "demo.rhai".into(),
                text: "fn hello() {}\n".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "import \"demo\" as d;\n\nfn run() {\n    d::hello();\n}\n".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "other.rhai".into(),
                text: "import \"demo\" as tools;\n".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let demo = analysis
        .db
        .vfs()
        .file_id(Path::new("demo.rhai"))
        .expect("expected demo.rhai");
    let other = analysis
        .db
        .vfs()
        .file_id(Path::new("other.rhai"))
        .expect("expected other.rhai");
    let consumer_text = analysis
        .db
        .file_text(consumer)
        .expect("expected consumer text");
    let offset = u32::try_from(
        consumer_text
            .find("\"demo\"")
            .expect("expected import literal")
            + 1,
    )
    .expect("expected offset to fit");

    let rename = analysis
        .rename(
            FilePosition {
                file_id: consumer,
                offset,
            },
            "renamed_demo",
        )
        .expect("expected prepared rename");

    assert!(rename.plan.issues.is_empty());
    let source_change = rename
        .source_change
        .expect("expected concrete source change");
    assert_eq!(source_change.file_renames.len(), 1);
    assert_eq!(source_change.file_renames[0].file_id, demo);
    assert_eq!(
        source_change.file_renames[0].new_path,
        normalize_path(Path::new("renamed_demo.rhai"))
    );

    let consumer_edits = source_change
        .file_edits
        .iter()
        .find(|edit| edit.file_id == consumer)
        .expect("expected consumer edits");
    assert_eq!(consumer_edits.edits.len(), 1);
    assert_eq!(consumer_edits.edits[0].new_text, "\"renamed_demo\"");

    let other_edits = source_change
        .file_edits
        .iter()
        .find(|edit| edit.file_id == other)
        .expect("expected other edits");
    assert_eq!(other_edits.edits.len(), 1);
    assert_eq!(other_edits.edits[0].new_text, "\"renamed_demo\"");
}
#[test]
fn renaming_static_import_module_reference_preserves_path_prefixes() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "shared/demo.rhai".into(),
                text: "fn hello() {}\n".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "import \"shared/demo\" as d;\n".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "nested.rhai".into(),
                text: "import \"a/b/c\" as d;\n".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let provider = analysis
        .db
        .vfs()
        .file_id(Path::new("shared/demo.rhai"))
        .expect("expected provider file");
    let consumer_text = analysis
        .db
        .file_text(consumer)
        .expect("expected consumer text");
    let offset = u32::try_from(
        consumer_text
            .find("\"shared/demo\"")
            .expect("expected import literal")
            + 8,
    )
    .expect("expected offset to fit");

    let rename = analysis
        .rename(
            FilePosition {
                file_id: consumer,
                offset,
            },
            "renamed",
        )
        .expect("expected prepared rename");

    assert!(rename.plan.issues.is_empty());
    let source_change = rename
        .source_change
        .expect("expected concrete source change");
    assert_eq!(source_change.file_renames.len(), 1);
    assert_eq!(source_change.file_renames[0].file_id, provider);
    assert_eq!(
        source_change.file_renames[0].new_path,
        normalize_path(Path::new("shared/renamed.rhai"))
    );
    let consumer_edits = source_change
        .file_edits
        .iter()
        .find(|edit| edit.file_id == consumer)
        .expect("expected consumer edits");
    assert_eq!(consumer_edits.edits.len(), 1);
    assert_eq!(consumer_edits.edits[0].new_text, "\"shared/renamed\"");
}
#[test]
fn renaming_static_import_module_reference_rejects_new_path_segments() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "demo.rhai".into(),
                text: "fn hello() {}\n".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "import \"demo\" as d;\n".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let consumer_text = analysis
        .db
        .file_text(consumer)
        .expect("expected consumer text");
    let offset = u32::try_from(
        consumer_text
            .find("\"demo\"")
            .expect("expected import literal")
            + 1,
    )
    .expect("expected offset to fit");

    let rename = analysis
        .rename(
            FilePosition {
                file_id: consumer,
                offset,
            },
            "other/path",
        )
        .expect("expected prepared rename");

    assert!(rename.source_change.is_none());
    assert!(
        rename.plan.issues.iter().any(|issue| issue
            .message
            .contains("only supports changing the file name")),
        "expected path-segment rename issue, got {:?}",
        rename.plan.issues
    );
}
