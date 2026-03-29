use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, FilePosition, ReferenceKind};

#[test]
fn references_and_rename_plan_for_exports_stay_local_to_definition_file() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "provider.rhai",
        r#"
            let helper = 1;
            export helper as shared_tools;
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let provider = analysis
        .db
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    assert_no_syntax_diagnostics(&analysis, provider);
    let provider_text = analysis
        .db
        .file_text(provider)
        .expect("expected provider text");
    let offset = u32::try_from(
        provider_text
            .find("shared_tools")
            .expect("expected shared_tools"),
    )
    .expect("expected offset to fit");

    let references = analysis
        .find_references(FilePosition {
            file_id: provider,
            offset,
        })
        .expect("expected references result");
    assert_eq!(references.targets.len(), 1);
    assert_eq!(references.references.len(), 1);
    assert_eq!(references.references[0].kind, ReferenceKind::Definition);

    let rename = analysis
        .rename_plan(
            FilePosition {
                file_id: provider,
                offset,
            },
            "renamed_tools",
        )
        .expect("expected rename plan");
    assert_eq!(rename.new_name, "renamed_tools");
    assert_eq!(rename.occurrences.len(), 1);
    assert_eq!(rename.occurrences[0].kind, ReferenceKind::Definition);
}

#[test]
fn rename_materializes_export_alias_source_changes() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "provider.rhai",
        r#"
            let helper = 1;
            export helper as shared_tools;
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let provider = analysis
        .db
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    assert_no_syntax_diagnostics(&analysis, provider);
    let provider_text = analysis
        .db
        .file_text(provider)
        .expect("expected provider text");
    let offset = u32::try_from(
        provider_text
            .find("shared_tools")
            .expect("expected shared_tools"),
    )
    .expect("expected offset to fit");

    let rename = analysis
        .rename(
            FilePosition {
                file_id: provider,
                offset,
            },
            "renamed_tools",
        )
        .expect("expected prepared rename");

    assert!(rename.plan.issues.is_empty());
    let source_change = rename
        .source_change
        .expect("expected concrete source change");
    assert_eq!(source_change.file_edits.len(), 1);
    let provider_edits = source_change
        .file_edits
        .iter()
        .find(|edit| edit.file_id == provider)
        .expect("expected provider edits");

    assert_eq!(provider_edits.edits.len(), 1);
    assert_eq!(provider_edits.edits[0].new_text, "renamed_tools");
}

#[test]
fn rename_with_export_conflicts_reports_issues_without_materializing_edits() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    let helper = 1;
                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "existing.rhai".into(),
                text: r#"
                    let helper = 1;
                    export helper as renamed_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let analysis = host.snapshot();
    let provider = analysis
        .db
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let existing = analysis
        .db
        .vfs()
        .file_id(Path::new("existing.rhai"))
        .expect("expected existing.rhai");
    assert_no_syntax_diagnostics(&analysis, provider);
    assert_no_syntax_diagnostics(&analysis, existing);
    let provider_text = analysis
        .db
        .file_text(provider)
        .expect("expected provider text");
    let offset = u32::try_from(
        provider_text
            .find("shared_tools")
            .expect("expected shared_tools"),
    )
    .expect("expected offset to fit");

    let rename = analysis
        .rename(
            FilePosition {
                file_id: provider,
                offset,
            },
            "renamed_tools",
        )
        .expect("expected prepared rename");

    assert!(!rename.plan.issues.is_empty());
    assert!(rename.source_change.is_none());
}
