use std::path::Path;

use rhai_db::ChangeSet;
use rhai_db::ProjectDiagnosticCode;
use rhai_hir::SemanticDiagnosticCode;
use rhai_vfs::DocumentVersion;

use crate::AnalysisHost;
use crate::tests::assert_no_syntax_diagnostics;

#[test]
fn diagnostics_report_unresolved_non_string_import_modules() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "consumer.rhai",
        "import shared_tools as tools;",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, consumer);

    assert!(analysis.diagnostics(consumer).iter().any(|diagnostic| {
        diagnostic.code
            == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedImportModule)
    }));
}
#[test]
fn diagnostics_report_non_string_import_module_expressions() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "consumer.rhai",
        r#"
            fn helper() {}
            import helper as tools;
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, consumer);

    assert!(analysis.diagnostics(consumer).iter().any(|diagnostic| {
        diagnostic.code
            == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::InvalidImportModuleType)
    }));
}
#[test]
fn diagnostics_report_unresolved_import_members_after_provider_renames() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() { 1 }
                    export const VALUE = 1;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
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
        project: Some(rhai_project::ProjectConfig::default()),
    });

    host.apply_change(ChangeSet::single_file(
        "provider.rhai",
        r#"
            fn renamed_helper() { 1 }
            export const RENAMED = 1;
        "#,
        DocumentVersion(2),
    ));

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, consumer);

    let diagnostics = analysis.diagnostics(consumer);
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
fn diagnostics_report_missing_static_path_import_files() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            import "./missing_module" as missing;
            fn run() {}
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

    assert!(
        analysis
            .diagnostics(file_id)
            .iter()
            .any(|diagnostic| { diagnostic.code == ProjectDiagnosticCode::BrokenLinkedImport })
    );
}
#[test]
fn diagnostics_report_unresolved_static_named_import_modules() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            import "env" as env;
            fn run() {}
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

    assert!(analysis.diagnostics(file_id).iter().any(|diagnostic| {
        diagnostic.code
            == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedImportModule)
    }));
}
#[test]
fn diagnostics_allow_global_import_aliases_inside_functions() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            import "hello" as hey;

            fn helper(value) {
                hey::process(value);
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

    assert!(!analysis.diagnostics(file_id).iter().any(|diagnostic| {
        diagnostic.code == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
    }));
}
#[test]
fn diagnostics_keep_unresolved_non_string_import_modules_visible() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "consumer.rhai",
        "import shared_tools as tools;\n\nfn run() {}",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, consumer);

    assert!(analysis.diagnostics(consumer).iter().any(|diagnostic| {
        diagnostic.code
            == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedImportModule)
    }));
}
#[test]
fn diagnostics_keep_unaliased_import_regular_members_unresolved() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    export const VALUE = 1;

                    fn helper(value) {
                        value
                    }

                    fn int.bump(delta) {
                        this + delta
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider";

                    fn run() {
                        let _a = helper(1);
                        let _b = VALUE;
                        let _c = 1.bump(2);
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(rhai_project::ProjectConfig::default()),
    });

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, consumer);

    let diagnostics = analysis.diagnostics(consumer);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
    }));
    assert!(
        diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code
                    == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
            })
            .count()
            == 2
    );
}
