use std::path::Path;

use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, offset_in};
use crate::{AnalyzerDatabase, ChangeSet, FileChange};
use rhai_vfs::DocumentVersion;

#[test]
fn workspace_symbol_search_supports_project_wide_queries() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "alpha.rhai".into(),
                text: r#"
                    fn helper() {}
                    let api_value = 1;
                    export api_value as public_helper;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "beta.rhai".into(),
                text: r#"
                    fn helper_tool() {}
                    fn Worker() { helper_tool(); }
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
    let helper_matches = snapshot.workspace_symbols_matching("helper");
    assert_eq!(
        helper_matches
            .iter()
            .map(|symbol| (
                symbol.file_id,
                symbol.symbol.name.as_str(),
                symbol.symbol.exported
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                snapshot
                    .vfs()
                    .file_id(Path::new("alpha.rhai"))
                    .expect("expected alpha.rhai"),
                "helper",
                true,
            ),
            (
                snapshot
                    .vfs()
                    .file_id(Path::new("beta.rhai"))
                    .expect("expected beta.rhai"),
                "helper_tool",
                true,
            ),
            (
                snapshot
                    .vfs()
                    .file_id(Path::new("alpha.rhai"))
                    .expect("expected alpha.rhai"),
                "public_helper",
                true,
            ),
        ]
    );

    let worker_matches = snapshot.workspace_symbols_matching("worker");
    assert_eq!(worker_matches.len(), 1);
    assert_eq!(worker_matches[0].symbol.name, "Worker");
}
#[test]
fn completion_inputs_collect_visible_member_and_project_symbols() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "main.rhai".into(),
                text: r#"
                    fn helper() {}
                    fn run() {
                        let user = #{ name: "Ada", id: 42 };
                        let text = "Ada";
                        let local_value = 1;
                        user.
                        text.
                        helper();
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "support.rhai".into(),
                text: r#"
                    let hidden_value = 1;
                    fn shared_helper() {}
                    fn project_only() {}
                    export shared_helper as shared_helper;
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
    let main = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let main_text = snapshot.file_text(main).expect("expected main text");

    let helper_offset = offset_in(&main_text, "helper();");
    let helper_inputs = snapshot
        .completion_inputs(main, helper_offset)
        .expect("expected completion inputs");
    assert!(
        helper_inputs
            .visible_symbols
            .iter()
            .any(|symbol| symbol.name == "helper")
    );
    assert!(
        helper_inputs
            .visible_symbols
            .iter()
            .any(|symbol| symbol.name == "local_value")
    );
    assert!(
        helper_inputs
            .project_symbols
            .iter()
            .any(|symbol| symbol.symbol.name == "shared_helper")
    );
    assert!(
        !helper_inputs
            .project_symbols
            .iter()
            .any(|symbol| symbol.symbol.name == "hidden_value")
    );
    assert!(
        !helper_inputs
            .project_symbols
            .iter()
            .any(|symbol| symbol.symbol.name == "helper")
    );

    let member_offset = offset_in(&main_text, "user.");
    let member_inputs = snapshot
        .completion_inputs(main, member_offset)
        .expect("expected member completion inputs");
    assert!(
        !member_inputs.member_symbols.is_empty(),
        "expected member completions for object literal fields"
    );
    assert!(
        member_inputs
            .member_symbols
            .iter()
            .any(|member| member.name == "name")
            || member_inputs
                .member_symbols
                .iter()
                .any(|member| member.name == "id")
    );

    let string_member_offset =
        offset_in(&main_text, "text.") + rhai_syntax::TextSize::from("text.".len() as u32);
    let string_member_inputs = snapshot
        .completion_inputs(main, string_member_offset)
        .expect("expected string member completion inputs");
    assert!(
        string_member_inputs
            .member_symbols
            .iter()
            .any(|member| member.name == "contains")
    );
    assert!(
        string_member_inputs
            .member_symbols
            .iter()
            .any(|member| member.name == "len")
    );
}
#[test]
fn auto_import_candidates_do_not_plan_symbol_imports_from_workspace_exports() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: "let helper = 1; export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: "fn run() { shared_tools(); }".to_owned(),
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
    let consumer_text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");

    let candidates =
        snapshot.auto_import_candidates(consumer, offset_in(&consumer_text, "shared_tools"));
    assert!(candidates.is_empty());
}
