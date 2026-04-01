use std::path::Path;

use rhai_db::{ChangeSet, FileChange};
use rhai_project::{FunctionSpec, ModuleSpec, ProjectConfig, TypeSpec};
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, SemanticTokenKind, SemanticTokenModifier};

#[test]
fn semantic_tokens_mark_builtin_and_host_symbols_as_default_library_and_keep_imported_calls_precise()
 {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
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
                path: "main.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        let widget = math::make_widget();
                        blob(1);
                        math::add(1, 2);
                        "abc".len();
                        widget.open();
                        tools::helper();
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            modules: [(
                "math".to_owned(),
                ModuleSpec {
                    functions: [
                        (
                            "add".to_owned(),
                            vec![FunctionSpec {
                                signature: "fun(int, int) -> int".to_owned(),
                                return_type: None,
                                docs: None,
                            }],
                        ),
                        (
                            "make_widget".to_owned(),
                            vec![FunctionSpec {
                                signature: "fun() -> Widget".to_owned(),
                                return_type: None,
                                docs: None,
                            }],
                        ),
                    ]
                    .into_iter()
                    .collect(),
                    ..ModuleSpec::default()
                },
            )]
            .into_iter()
            .collect(),
            types: [(
                "Widget".to_owned(),
                TypeSpec {
                    methods: [(
                        "open".to_owned(),
                        vec![FunctionSpec {
                            signature: "fun() -> int".to_owned(),
                            return_type: None,
                            docs: None,
                        }],
                    )]
                    .into_iter()
                    .collect(),
                    ..TypeSpec::default()
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
    let text = analysis.file_text(file_id).expect("expected text");
    let tokens = analysis.semantic_tokens(file_id);

    let find = |needle: &str, kind: SemanticTokenKind| {
        tokens.iter().find(|token| {
            let start = u32::from(token.range.start()) as usize;
            let end = u32::from(token.range.end()) as usize;
            &text[start..end] == needle && token.kind == kind
        })
    };

    let blob = find("blob", SemanticTokenKind::Function).expect("expected blob token");
    assert!(
        blob.modifiers
            .contains(&SemanticTokenModifier::DefaultLibrary)
    );

    let math = find("math", SemanticTokenKind::Namespace).expect("expected math token");
    assert!(
        math.modifiers
            .contains(&SemanticTokenModifier::DefaultLibrary)
    );

    let add = find("add", SemanticTokenKind::Function).expect("expected add token");
    assert!(
        add.modifiers
            .contains(&SemanticTokenModifier::DefaultLibrary)
    );

    let len = find("len", SemanticTokenKind::Method).expect("expected len token");
    assert!(
        len.modifiers
            .contains(&SemanticTokenModifier::DefaultLibrary)
    );

    let open = find("open", SemanticTokenKind::Method).expect("expected open token");
    assert!(
        open.modifiers
            .contains(&SemanticTokenModifier::DefaultLibrary)
    );

    let tools = find("tools", SemanticTokenKind::Namespace).expect("expected tools token");
    assert!(
        !tools
            .modifiers
            .contains(&SemanticTokenModifier::DefaultLibrary)
    );

    let helper = find("helper", SemanticTokenKind::Function).expect("expected helper token");
    assert!(
        !helper
            .modifiers
            .contains(&SemanticTokenModifier::DefaultLibrary)
    );
}
