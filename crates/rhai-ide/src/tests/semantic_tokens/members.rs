use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, SemanticTokenKind, SemanticTokenModifier};

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
