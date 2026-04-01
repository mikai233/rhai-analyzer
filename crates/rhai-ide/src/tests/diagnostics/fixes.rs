use std::path::Path;

use rhai_db::ChangeSet;
use rhai_db::ProjectDiagnosticCode;
use rhai_hir::SemanticDiagnosticCode;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, DiagnosticSeverity, DiagnosticTag};

#[test]
fn diagnostics_with_fixes_do_not_attach_workspace_export_auto_imports() {
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

    let diagnostics = analysis.diagnostics_with_fixes(consumer);
    let unresolved = diagnostics
        .iter()
        .find(|entry| {
            entry.diagnostic.code
                == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
        })
        .expect("expected unresolved name diagnostic");

    assert!(unresolved.fixes.is_empty());
}
#[test]
fn diagnostics_with_fixes_attach_remove_unused_import_fix() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "import \"shared_tools\" as tools;\nfn run() {}",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let diagnostics = analysis.diagnostics_with_fixes(file_id);
    let unused = diagnostics
        .iter()
        .find(|entry| {
            entry.diagnostic.code
                == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnusedSymbol)
        })
        .expect("expected unused import diagnostic");

    assert_eq!(unused.fixes.len(), 1);
    assert_eq!(unused.fixes[0].id.as_str(), "import.remove_unused");
    assert_eq!(unused.fixes[0].label, "Remove unused import");
    assert_eq!(unused.diagnostic.severity, DiagnosticSeverity::Warning);
    assert_eq!(unused.diagnostic.tags, vec![DiagnosticTag::Unnecessary]);
}
#[test]
fn diagnostics_keep_errors_for_hard_failures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 42;

            fn helper() {
                value
            }

            helper();
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

    let unresolved = analysis
        .diagnostics(file_id)
        .into_iter()
        .find(|diagnostic| {
            diagnostic.code
                == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
        })
        .expect("expected unresolved name diagnostic");

    assert_eq!(unresolved.severity, DiagnosticSeverity::Error);
    assert!(unresolved.tags.is_empty());
}
