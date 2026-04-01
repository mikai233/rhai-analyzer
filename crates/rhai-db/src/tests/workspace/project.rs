use crate::tests::assert_workspace_files_have_no_syntax_diagnostics;
use crate::{AnalyzerDatabase, ChangeSet, FileChange};
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;
use std::path::Path;
use std::ptr;
use std::sync::Arc;

#[test]
fn project_changes_refresh_project_semantics() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: Vec::new(),
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            modules: [(
                "math".to_owned(),
                rhai_project::ModuleSpec {
                    docs: None,
                    functions: [(
                        "add".to_owned(),
                        vec![rhai_project::FunctionSpec {
                            signature: "fun(int, int) -> int".to_owned(),
                            return_type: None,
                            docs: None,
                        }],
                    )]
                    .into_iter()
                    .collect(),
                    constants: Default::default(),
                },
            )]
            .into_iter()
            .collect(),
            ..ProjectConfig::default()
        }),
    });

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let first_signatures = first_snapshot.external_signatures();
    assert!(first_signatures.get("math::add").is_some());

    db.apply_change(ChangeSet {
        files: Vec::new(),
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            engine: rhai_project::EngineOptions {
                disabled_symbols: vec!["spawn".to_owned()],
                custom_syntaxes: vec!["unless".to_owned()],
            },
            modules: [(
                "io".to_owned(),
                rhai_project::ModuleSpec {
                    docs: None,
                    functions: [(
                        "read".to_owned(),
                        vec![rhai_project::FunctionSpec {
                            signature: "fun(string) -> string".to_owned(),
                            return_type: None,
                            docs: None,
                        }],
                    )]
                    .into_iter()
                    .collect(),
                    constants: Default::default(),
                },
            )]
            .into_iter()
            .collect(),
            ..ProjectConfig::default()
        }),
    });

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    assert!(!ptr::eq(
        first_signatures,
        second_snapshot.external_signatures()
    ));
    assert_eq!(second_snapshot.disabled_symbols(), ["spawn"]);
    assert_eq!(second_snapshot.custom_syntaxes(), ["unless"]);
    assert!(
        second_snapshot
            .external_signatures()
            .get("math::add")
            .is_none()
    );
    assert!(
        second_snapshot
            .external_signatures()
            .get("io::read")
            .is_some()
    );
}
#[test]
fn project_changes_invalidate_all_cached_file_analysis() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "one.rhai".into(),
                text: "let first = 1;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "two.rhai".into(),
                text: "let second = 2;".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let one = first_snapshot
        .vfs()
        .file_id(Path::new("one.rhai"))
        .expect("expected one.rhai");
    let two = first_snapshot
        .vfs()
        .file_id(Path::new("two.rhai"))
        .expect("expected two.rhai");
    let first_one_hir = first_snapshot.hir(one).expect("expected hir");
    let first_two_hir = first_snapshot.hir(two).expect("expected hir");

    db.apply_change(ChangeSet {
        files: Vec::new(),
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            root: "workspace".into(),
            ..ProjectConfig::default()
        }),
    });

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    let second_one_hir = second_snapshot.hir(one).expect("expected hir");
    let second_two_hir = second_snapshot.hir(two).expect("expected hir");

    assert!(!Arc::ptr_eq(&first_one_hir, &second_one_hir));
    assert!(!Arc::ptr_eq(&first_two_hir, &second_two_hir));
}
