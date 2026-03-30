use std::collections::BTreeMap;
use std::path::Path;

use rhai_db::ChangeSet;
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, InlayHintKind};

#[test]
fn inlay_hints_show_inferred_variable_and_function_return_types() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn helper() {
                1
            }

            fn run() {
                let value = helper();
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
    let text = analysis.db.file_text(file_id).expect("expected text");
    let hints = analysis.inlay_hints(file_id);

    let labels_by_offset = hints
        .into_iter()
        .map(|hint| ((hint.offset, hint.kind), hint.label))
        .collect::<BTreeMap<_, _>>();

    let helper_return_offset = u32::try_from(
        text.find("{\n                1")
            .expect("expected helper body"),
    )
    .expect("offset");
    let value_offset =
        u32::try_from(text.find("value =").expect("expected value binding") + "value".len())
            .expect("offset");

    assert_eq!(
        labels_by_offset.get(&(helper_return_offset, InlayHintKind::Type)),
        Some(&" -> int".to_owned())
    );
    assert_eq!(
        labels_by_offset.get(&(value_offset, InlayHintKind::Type)),
        Some(&": int".to_owned())
    );
}

#[test]
fn inlay_hints_show_inferred_closure_parameter_and_return_types() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "main.rhai".into(),
                text: r#"
                    import "tools" as tools;

                    fn run() {
                        let result = tools::map_one(1, |item| {
                            item.to_float()
                        });
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "tools.rhai".into(),
                text: r#"
                    /// @param value T
                    /// @param mapper fun(T) -> U
                    /// @return U
                    fn map_one(value, mapper) {
                        mapper(value)
                    }
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
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let hints = analysis.inlay_hints(file_id);

    assert!(
        hints
            .iter()
            .any(|hint| { hint.kind == InlayHintKind::Type && hint.label == " -> float" }),
        "expected closure return hint, got {hints:?}"
    );

    let labels_by_offset = hints
        .into_iter()
        .map(|hint| ((hint.offset, hint.kind), hint.label))
        .collect::<BTreeMap<_, _>>();

    let closure_param_start = text.find("|item|").expect("expected closure param");
    let item_offset = u32::try_from(closure_param_start + "|item".len()).expect("offset");
    let result_offset =
        u32::try_from(text.find("result =").expect("expected result binding") + "result".len())
            .expect("offset");

    assert_eq!(
        labels_by_offset.get(&(item_offset, InlayHintKind::Type)),
        Some(&": int".to_owned())
    );
    assert_eq!(
        labels_by_offset.get(&(result_offset, InlayHintKind::Type)),
        Some(&": float".to_owned())
    );
}
