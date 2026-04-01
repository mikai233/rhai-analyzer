use crate::tests::assert_workspace_files_have_no_syntax_diagnostics;
use crate::{AnalyzerDatabase, ChangeSet};
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;
use std::path::Path;
use std::ptr;
use std::sync::Arc;

#[test]
fn unchanged_files_reuse_cached_analysis_across_snapshots() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = 1;",
        DocumentVersion(1),
    ));

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let file_id = first_snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let first_parse = first_snapshot.parse(file_id).expect("expected parse");
    let first_hir = first_snapshot.hir(file_id).expect("expected hir");

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    let second_parse = second_snapshot.parse(file_id).expect("expected parse");
    let second_hir = second_snapshot.hir(file_id).expect("expected hir");

    assert!(Arc::ptr_eq(&first_parse, &second_parse));
    assert!(Arc::ptr_eq(&first_hir, &second_hir));
}
#[test]
fn unchanged_files_reuse_cached_indexes_across_snapshots() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let sample = 1;
            export sample as public_sample;
        "#,
        DocumentVersion(1),
    ));

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let file_id = first_snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let first_index = first_snapshot
        .file_symbol_index(file_id)
        .expect("expected file symbol index");
    let first_module_graph = first_snapshot
        .module_graph(file_id)
        .expect("expected module graph");

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    let second_index = second_snapshot
        .file_symbol_index(file_id)
        .expect("expected file symbol index");
    let second_module_graph = second_snapshot
        .module_graph(file_id)
        .expect("expected module graph");

    assert!(Arc::ptr_eq(&first_index, &second_index));
    assert!(Arc::ptr_eq(&first_module_graph, &second_module_graph));
}
#[test]
fn file_changes_reuse_project_semantics() {
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
    let first_modules = first_snapshot.host_modules();

    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = 1;",
        DocumentVersion(1),
    ));

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);

    assert!(ptr::eq(
        first_signatures,
        second_snapshot.external_signatures()
    ));
    assert!(ptr::eq(first_modules, second_snapshot.host_modules()));
}
