use std::collections::BTreeMap;
use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, SemanticTokenKind};

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
