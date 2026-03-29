use std::path::Path;

use rhai_db::ChangeSet;
use rhai_syntax::{TextRange, TextSize};
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, AssistKind, FilePosition};
#[test]
fn auto_import_actions_are_not_returned_for_workspace_exports() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: "let helper = 1; export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "fn run() { shared_tools(); }".to_owned(),
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
    assert_no_syntax_diagnostics(&analysis, consumer);
    let offset = u32::try_from(
        analysis
            .db
            .file_text(consumer)
            .expect("expected consumer text")
            .find("shared_tools")
            .expect("expected unresolved reference"),
    )
    .expect("expected offset to fit");

    let actions = analysis.auto_import_actions(FilePosition {
        file_id: consumer,
        offset,
    });

    assert!(actions.is_empty());
}

#[test]
fn assists_do_not_offer_auto_import_quick_fixes_for_workspace_exports() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: "let helper = 1; export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "fn run() { shared_tools(); }".to_owned(),
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
    assert_no_syntax_diagnostics(&analysis, consumer);
    let offset = u32::try_from(
        analysis
            .db
            .file_text(consumer)
            .expect("expected consumer text")
            .find("shared_tools")
            .expect("expected unresolved reference"),
    )
    .expect("expected offset to fit");

    let assists = analysis.assists(
        consumer,
        TextRange::new(TextSize::from(offset), TextSize::from(offset)),
    );

    assert!(assists.is_empty());
}

#[test]
fn remove_unused_imports_plans_deletions_for_unused_aliases() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "import \"used_tools\" as used;\nimport \"unused_tools\" as unused;\nused;\nfn run() {}",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let change = analysis
        .remove_unused_imports(file_id)
        .expect("expected unused import cleanup");

    assert_eq!(change.file_edits.len(), 1);
    assert_eq!(change.file_edits[0].file_id, file_id);
    assert_eq!(change.file_edits[0].edits.len(), 1);
    assert_eq!(change.file_edits[0].edits[0].new_text, "");
}

#[test]
fn organize_imports_sorts_deduplicates_and_normalizes_import_block() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "import \"zebra\" as zebra;\nimport \"alpha\" as alpha;\nimport \"zebra\" as zebra;\nfn run() {}",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let change = analysis
        .organize_imports(file_id)
        .expect("expected organize imports change");

    assert_eq!(change.file_edits.len(), 1);
    assert_eq!(change.file_edits[0].edits.len(), 1);
    assert_eq!(
        change.file_edits[0].edits[0].new_text,
        "import \"alpha\" as alpha;\nimport \"zebra\" as zebra;"
    );
}

#[test]
fn assists_include_source_import_cleanup_actions_for_import_blocks() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "import \"zebra\" as zebra;\nimport \"alpha\" as alpha;\nimport \"zebra\" as zebra;\nfn run() {}",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let assists = analysis.assists(
        file_id,
        TextRange::new(TextSize::from(0), TextSize::from(0)),
    );

    assert!(assists.iter().any(|assist| {
        assist.id.as_str() == "import.organize"
            && assist.kind == AssistKind::Source
            && assist.label == "Organize imports"
    }));
}
