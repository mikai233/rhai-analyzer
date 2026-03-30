use std::path::Path;

use rhai_db::{ChangeSet, FileChange};
use rhai_project::{FunctionSpec, ModuleSpec, ProjectConfig};
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, FoldingRangeKind};

#[test]
fn folding_ranges_cover_comments_functions_and_containers() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// docs line 1
            /// docs line 2
            fn helper() {
                let values = [
                    1,
                    2,
                ];
                let user = #{
                    name: "Ada",
                    nested: #{
                        value: 1,
                    },
                };
                if true {
                    values.len();
                }
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
    let text = analysis.file_text(file_id).expect("expected text");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let ranges = analysis.folding_ranges(file_id);
    let folded = ranges
        .iter()
        .map(|range| {
            let start = u32::from(range.range.start()) as usize;
            let end = u32::from(range.range.end()) as usize;
            (text[start..end].trim().to_owned(), range.kind)
        })
        .collect::<Vec<_>>();

    assert!(
        folded.iter().any(|(snippet, kind)| {
            snippet.contains("docs line 1")
                && snippet.contains("docs line 2")
                && *kind == FoldingRangeKind::Comment
        }),
        "expected folded comment block, got {folded:?}"
    );
    assert!(
        folded
            .iter()
            .any(|(snippet, kind)| snippet.starts_with("fn helper()")
                && *kind == FoldingRangeKind::Region),
        "expected folded function, got {folded:?}"
    );
    assert!(
        folded.iter().any(|(snippet, kind)| snippet.starts_with("[")
            && snippet.contains("1,")
            && *kind == FoldingRangeKind::Region),
        "expected folded array literal, got {folded:?}"
    );
    assert!(
        folded
            .iter()
            .any(|(snippet, kind)| snippet.starts_with("#{")
                && snippet.contains("nested")
                && *kind == FoldingRangeKind::Region),
        "expected folded object literal, got {folded:?}"
    );
    assert!(
        folded
            .iter()
            .any(|(snippet, kind)| snippet.starts_with("if true")
                && *kind == FoldingRangeKind::Region),
        "expected folded if expression, got {folded:?}"
    );
}

#[test]
fn folding_ranges_cover_switch_and_imported_call_arguments() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                fn run() {
                    switch type_of(math::pick(
                        1,
                        2,
                    )) {
                        "int" => {
                            print("ok");
                        },
                        _ => {
                            print("fallback");
                        }
                    }
                }
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            modules: [(
                "math".to_owned(),
                ModuleSpec {
                    functions: [(
                        "pick".to_owned(),
                        vec![FunctionSpec {
                            signature: "fun(int, int) -> int".to_owned(),
                            return_type: None,
                            docs: None,
                        }],
                    )]
                    .into_iter()
                    .collect(),
                    ..ModuleSpec::default()
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
    let text = analysis.file_text(file_id).expect("expected text");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let folded = analysis
        .folding_ranges(file_id)
        .into_iter()
        .map(|range| {
            let start = u32::from(range.range.start()) as usize;
            let end = u32::from(range.range.end()) as usize;
            (text[start..end].trim().to_owned(), range.kind)
        })
        .collect::<Vec<_>>();

    assert!(
        folded
            .iter()
            .any(|(snippet, _)| snippet.starts_with("math::pick(") && snippet.contains("1,")),
        "expected folded multiline call arguments, got {folded:?}"
    );
    assert!(
        folded
            .iter()
            .any(|(snippet, _)| snippet.starts_with("switch type_of(")
                && snippet.contains("\"int\" =>")),
        "expected folded switch expression, got {folded:?}"
    );
}
