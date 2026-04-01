use std::path::Path;

use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, offset_in};
use crate::{AnalyzerDatabase, ChangeSet, FileChange, ProjectDiagnosticCode};
use rhai_hir::SemanticDiagnosticCode;
use rhai_project::ProjectConfig;
use rhai_syntax::TextSize;
use rhai_vfs::DocumentVersion;

#[test]
fn imported_member_references_report_unresolved_after_provider_renames() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() { 1 }
                    export const VALUE = 1;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;
                    fn run() {
                        tools::helper();
                        let value = tools::VALUE;
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(ProjectConfig::default()),
    });

    db.apply_change(ChangeSet::single_file(
        "provider.rhai",
        r#"
            fn renamed_helper() { 1 }
            export const RENAMED = 1;
        "#,
        DocumentVersion(2),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");

    let diagnostics = snapshot.project_diagnostics(consumer);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == ProjectDiagnosticCode::UnresolvedImportMember)
    );
    assert!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == ProjectDiagnosticCode::UnresolvedImportMember)
            .count()
            >= 2
    );
}
#[test]
fn static_path_imports_report_missing_workspace_files() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            import "./missing_module" as missing;
            fn run() {}
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");

    let diagnostics = snapshot.project_diagnostics(file_id);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == ProjectDiagnosticCode::BrokenLinkedImport)
    );
}
#[test]
fn static_named_imports_report_unresolved_modules() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            import "env" as env;
            fn run() {}
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");

    let diagnostics = snapshot.project_diagnostics(file_id);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code
            == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedImportModule)
    }));
}
#[test]
fn changing_exports_does_not_break_static_import_linkage() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    let helper = 1;
                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"import "provider" as tools;"#.to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let consumer = first_snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_eq!(first_snapshot.linked_imports(consumer).len(), 1);

    db.apply_change(ChangeSet::single_file(
        "provider.rhai",
        r#"
            let helper = 1;
            export helper as renamed_tools;
        "#,
        DocumentVersion(2),
    ));

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    assert_eq!(second_snapshot.linked_imports(consumer).len(), 1);
    assert!(second_snapshot.exports_named("shared_tools").is_empty());
    assert_eq!(second_snapshot.exports_named("renamed_tools").len(), 1);
}
#[test]
fn change_report_surfaces_dependency_affected_files_for_static_imports() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: "export const VALUE = 1;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"import "provider" as tools;"#.to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let impact = db.apply_change_report(ChangeSet::single_file(
        "provider.rhai",
        "export const VALUE = 2;",
        DocumentVersion(2),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");

    assert_eq!(impact.changed_files, vec![provider]);
    assert_eq!(impact.rebuilt_files, vec![provider]);
    assert_eq!(impact.dependency_affected_files, vec![consumer]);
}
#[test]
fn change_report_marks_dependencies_affected_when_importers_change() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: "export const VALUE = 1;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"import "provider" as tools;"#.to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let impact = db.apply_change_report(ChangeSet::single_file(
        "consumer.rhai",
        "import \"provider\" as tools;\nfn run() {}",
        DocumentVersion(2),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");

    assert_eq!(impact.changed_files, vec![consumer]);
    assert_eq!(impact.rebuilt_files, vec![consumer]);
    assert_eq!(impact.dependency_affected_files, vec![provider]);
}
#[test]
fn goto_definition_on_import_module_reference_targets_provider_file() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() {
                        1
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer_text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");
    let module_offset = offset_in(&consumer_text, "\"provider\"") + TextSize::from(1);

    let definitions = snapshot.goto_definition(consumer, module_offset);

    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].file_id, provider);
}
