use crate::tests::assert_workspace_files_have_no_syntax_diagnostics;
use crate::{AnalyzerDatabase, ChangeSet, FileChange};
use rhai_vfs::DocumentVersion;
use std::path::Path;
use std::sync::Arc;

#[test]
fn text_changes_invalidate_only_affected_file_analysis() {
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
    let first_one_parse = first_snapshot.parse(one).expect("expected parse");
    let first_two_parse = first_snapshot.parse(two).expect("expected parse");

    db.apply_change(ChangeSet::single_file(
        "one.rhai",
        "let first = 10;",
        DocumentVersion(2),
    ));

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    let second_one_parse = second_snapshot.parse(one).expect("expected parse");
    let second_two_parse = second_snapshot.parse(two).expect("expected parse");

    assert!(!Arc::ptr_eq(&first_one_parse, &second_one_parse));
    assert!(Arc::ptr_eq(&first_two_parse, &second_two_parse));
}
#[test]
fn text_changes_invalidate_only_affected_file_indexes_and_refresh_workspace_symbols() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "one.rhai".into(),
                text: "fn alpha() {}".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "two.rhai".into(),
                text: "fn beta() {}".to_owned(),
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
    let first_one_index = first_snapshot
        .file_symbol_index(one)
        .expect("expected file symbol index");
    let first_two_index = first_snapshot
        .file_symbol_index(two)
        .expect("expected file symbol index");

    db.apply_change(ChangeSet::single_file(
        "one.rhai",
        "fn gamma() {}",
        DocumentVersion(2),
    ));

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    let second_one_index = second_snapshot
        .file_symbol_index(one)
        .expect("expected file symbol index");
    let second_two_index = second_snapshot
        .file_symbol_index(two)
        .expect("expected file symbol index");

    assert!(!Arc::ptr_eq(&first_one_index, &second_one_index));
    assert!(Arc::ptr_eq(&first_two_index, &second_two_index));
    assert_eq!(
        second_snapshot
            .workspace_symbols()
            .iter()
            .map(|symbol| (symbol.file_id, symbol.symbol.name.as_str()))
            .collect::<Vec<_>>(),
        vec![(two, "beta"), (one, "gamma")]
    );
}
#[test]
fn text_changes_refresh_workspace_module_graphs_and_symbol_locations() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let alpha = 1; export alpha as public_alpha;",
        DocumentVersion(1),
    ));

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let file_id = first_snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let first_graph = first_snapshot
        .module_graph(file_id)
        .expect("expected module graph");
    let first_target = first_graph.exports[0]
        .target
        .as_ref()
        .expect("expected exported target")
        .clone();

    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let beta = 2; export beta as public_beta;",
        DocumentVersion(2),
    ));

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    let second_graph = second_snapshot
        .module_graph(file_id)
        .expect("expected module graph");
    let second_target = second_graph.exports[0]
        .target
        .as_ref()
        .expect("expected exported target")
        .clone();

    assert_eq!(second_snapshot.workspace_module_graphs().len(), 1);
    assert!(second_snapshot.symbol_owner(&first_target).is_none());
    assert_eq!(second_snapshot.symbol_owner(&second_target), Some(file_id));
    assert_eq!(
        second_snapshot.locate_symbol(&second_target)[0].symbol.name,
        "beta"
    );
}
#[test]
fn workspace_index_invalidation_updates_only_changed_file_contributions() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "one.rhai".into(),
                text: "let alpha = 1; export alpha as public_alpha;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "two.rhai".into(),
                text: "let beta = 2; export beta as public_beta;".to_owned(),
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

    let first_one_symbols = Arc::clone(
        db.workspace_indexes
            .symbols_by_file
            .get(&one)
            .expect("expected one.rhai symbols"),
    );
    let first_two_symbols = Arc::clone(
        db.workspace_indexes
            .symbols_by_file
            .get(&two)
            .expect("expected two.rhai symbols"),
    );
    let first_one_graph = Arc::clone(
        db.workspace_indexes
            .module_graphs_by_file
            .get(&one)
            .expect("expected one.rhai module graph"),
    );
    let first_two_graph = Arc::clone(
        db.workspace_indexes
            .module_graphs_by_file
            .get(&two)
            .expect("expected two.rhai module graph"),
    );
    let first_one_locations = Arc::clone(
        db.workspace_indexes
            .symbol_locations_by_file
            .get(&one)
            .expect("expected one.rhai symbol locations"),
    );
    let first_two_locations = Arc::clone(
        db.workspace_indexes
            .symbol_locations_by_file
            .get(&two)
            .expect("expected two.rhai symbol locations"),
    );

    db.apply_change(ChangeSet::single_file(
        "one.rhai",
        "let gamma = 3; export gamma as public_gamma;",
        DocumentVersion(2),
    ));

    let second_one_symbols = db
        .workspace_indexes
        .symbols_by_file
        .get(&one)
        .expect("expected one.rhai symbols");
    let second_two_symbols = db
        .workspace_indexes
        .symbols_by_file
        .get(&two)
        .expect("expected two.rhai symbols");
    let second_one_graph = db
        .workspace_indexes
        .module_graphs_by_file
        .get(&one)
        .expect("expected one.rhai module graph");
    let second_two_graph = db
        .workspace_indexes
        .module_graphs_by_file
        .get(&two)
        .expect("expected two.rhai module graph");
    let second_one_locations = db
        .workspace_indexes
        .symbol_locations_by_file
        .get(&one)
        .expect("expected one.rhai symbol locations");
    let second_two_locations = db
        .workspace_indexes
        .symbol_locations_by_file
        .get(&two)
        .expect("expected two.rhai symbol locations");

    assert!(!Arc::ptr_eq(&first_one_symbols, second_one_symbols));
    assert!(Arc::ptr_eq(&first_two_symbols, second_two_symbols));
    assert!(!Arc::ptr_eq(&first_one_graph, second_one_graph));
    assert!(Arc::ptr_eq(&first_two_graph, second_two_graph));
    assert!(!Arc::ptr_eq(&first_one_locations, second_one_locations));
    assert!(Arc::ptr_eq(&first_two_locations, second_two_locations));
}
