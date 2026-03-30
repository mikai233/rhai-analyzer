use std::collections::BTreeMap;
use std::path::Path;

use rhai_db::{ChangeSet, FileChange};
use rhai_project::{FunctionSpec, ModuleSpec, ProjectConfig, TypeSpec};
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, SemanticTokenKind, SemanticTokenModifier};

#[test]
fn semantic_tokens_classify_keywords_symbols_and_literals() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// docs
            import "tools" as tools;

            fn helper(param) {
                let local = param + 42;
                let text = `hi ${local}`;
                tools::run(local);
                local
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
    let text = analysis.file_text(file_id).expect("expected text");
    let tokens = analysis.semantic_tokens(file_id);

    let kinds_by_text = tokens
        .into_iter()
        .map(|token| {
            let start = u32::from(token.range.start()) as usize;
            let end = u32::from(token.range.end()) as usize;
            ((start, end), (text[start..end].to_owned(), token.kind))
        })
        .collect::<BTreeMap<_, _>>();

    assert!(
        kinds_by_text
            .values()
            .any(|(text, kind)| { text == "/// docs" && *kind == SemanticTokenKind::Comment })
    );
    assert!(
        kinds_by_text
            .values()
            .any(|(text, kind)| { text == "import" && *kind == SemanticTokenKind::Keyword })
    );
    assert!(
        kinds_by_text
            .values()
            .any(|(text, kind)| { text == "\"tools\"" && *kind == SemanticTokenKind::String })
    );
    assert!(
        kinds_by_text
            .values()
            .any(|(text, kind)| { text == "42" && *kind == SemanticTokenKind::Number })
    );
    assert!(
        kinds_by_text
            .values()
            .any(|(text, kind)| { text == "helper" && *kind == SemanticTokenKind::Function })
    );
    assert!(
        kinds_by_text
            .values()
            .any(|(text, kind)| { text == "param" && *kind == SemanticTokenKind::Parameter })
    );
    assert!(
        kinds_by_text
            .values()
            .any(|(text, kind)| { text == "local" && *kind == SemanticTokenKind::Variable })
    );
    assert!(
        kinds_by_text
            .values()
            .any(|(text, kind)| { text == "tools" && *kind == SemanticTokenKind::Namespace })
    );
}

#[test]
fn semantic_tokens_classify_properties_methods_types_and_modifiers() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            const LIMIT = 1;

            fn "Custom".trimmed() {
                this
            }

            fn run() {
                let user = #{ name: "Ada" };
                user.name;
                user.name.len();
                LIMIT;
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
    let text = analysis.file_text(file_id).expect("expected text");
    let tokens = analysis.semantic_tokens(file_id);

    let find_token = |needle: &str, kind: SemanticTokenKind| {
        tokens.iter().find(|token| {
            let start = u32::from(token.range.start()) as usize;
            let end = u32::from(token.range.end()) as usize;
            &text[start..end] == needle && token.kind == kind
        })
    };

    let custom_type = find_token("\"Custom\"", SemanticTokenKind::Type)
        .expect("expected typed receiver semantic token");
    assert!(
        custom_type
            .modifiers
            .contains(&SemanticTokenModifier::Declaration)
    );

    let this_token =
        find_token("this", SemanticTokenKind::Variable).expect("expected this semantic token");
    assert!(this_token.modifiers.is_empty());

    let property_token =
        find_token("name", SemanticTokenKind::Property).expect("expected property semantic token");
    assert!(property_token.modifiers.is_empty());

    let method_token =
        find_token("len", SemanticTokenKind::Method).expect("expected method semantic token");
    assert!(
        method_token
            .modifiers
            .contains(&SemanticTokenModifier::DefaultLibrary)
    );

    let const_token =
        find_token("LIMIT", SemanticTokenKind::Variable).expect("expected constant semantic token");
    assert!(
        const_token
            .modifiers
            .contains(&SemanticTokenModifier::Declaration)
    );
    assert!(
        const_token
            .modifiers
            .contains(&SemanticTokenModifier::Readonly)
    );

    assert!(
        tokens.iter().any(|token| {
            let start = u32::from(token.range.start()) as usize;
            let end = u32::from(token.range.end()) as usize;
            &text[start..end] == "LIMIT"
                && token.modifiers.contains(&SemanticTokenModifier::Readonly)
        }),
        "expected at least one readonly LIMIT semantic token, got {tokens:?}"
    );
}

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
