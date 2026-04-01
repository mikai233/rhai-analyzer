use std::path::Path;

use rhai_db::{ChangeSet, FileChange};
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, FilePosition};

#[test]
fn hover_supports_host_module_import_aliases() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                import "env" as env;

                env::test(1);
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            modules: [(
                "env".to_owned(),
                rhai_project::ModuleSpec {
                    docs: Some("Environment helpers".to_owned()),
                    functions: [(
                        "test".to_owned(),
                        vec![rhai_project::FunctionSpec {
                            signature: "fun(int) -> int".to_owned(),
                            return_type: None,
                            docs: Some("Run the environment test".to_owned()),
                        }],
                    )]
                    .into_iter()
                    .collect(),
                    constants: [(
                        "DEFAULTS".to_owned(),
                        rhai_project::ConstantSpec {
                            type_name: "map<string, int>".to_owned(),
                            docs: Some("Default environment values".to_owned()),
                        },
                    )]
                    .into_iter()
                    .collect(),
                },
            )]
            .into_iter()
            .collect(),
            ..ProjectConfig::default()
        }),
    });

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset =
        u32::try_from(text.find("env;").expect("expected import alias") + 1).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected import alias hover");

    assert_eq!(hover.signature, r#"import "env" as env"#);
    assert_eq!(hover.docs.as_deref(), Some("Environment helpers"));
    assert!(
        hover
            .notes
            .iter()
            .any(|note| note == "Resolved from host module metadata.")
    );
    assert!(
        hover
            .notes
            .iter()
            .any(|note| note == "Module exposes 2 members.")
    );
}
#[test]
fn hover_supports_workspace_import_aliases() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper(value) { value }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    tools::helper(1);
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(ProjectConfig::default()),
    });

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset =
        u32::try_from(text.find("tools;").expect("expected import alias") + 1).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected workspace import alias hover");

    assert_eq!(hover.signature, r#"import "provider" as tools"#);
    assert!(hover.docs.is_none());
    assert!(
        hover
            .notes
            .iter()
            .any(|note| note.contains("provider.rhai"))
    );
}
#[test]
fn hover_supports_export_aliases() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let exported_value = 1;

            /// Public API alias
            export exported_value as public_value;
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
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.find("public_value").expect("expected export alias") + 1)
        .expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected export alias hover");

    assert_eq!(hover.signature, "export exported_value as public_value");
    assert_eq!(hover.docs.as_deref(), Some("Public API alias"));
    assert!(
        hover
            .notes
            .iter()
            .any(|note| note == "Re-exports: let exported_value")
    );
    assert!(
        hover
            .notes
            .iter()
            .any(|note| note == "Export alias is visible to importing modules.")
    );
}
