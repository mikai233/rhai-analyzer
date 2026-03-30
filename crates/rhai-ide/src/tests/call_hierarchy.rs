use std::path::Path;

use rhai_db::{ChangeSet, FileChange};
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, FilePosition};

#[test]
fn call_hierarchy_prepares_local_function_and_reports_incoming_and_outgoing_calls() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                fn leaf() {}

                fn middle() {
                    leaf();
                }

                fn root() {
                    middle();
                    leaf();
                }
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: None,
    });

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = analysis.file_text(file_id).expect("expected text");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let middle_offset =
        u32::try_from(text.find("middle()").expect("expected middle")).expect("offset");
    let item = analysis
        .prepare_call_hierarchy(FilePosition {
            file_id,
            offset: middle_offset,
        })
        .into_iter()
        .find(|item| item.name == "middle")
        .expect("expected middle call hierarchy item");

    let incoming = analysis.incoming_calls(&item);
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].from.name, "root");
    assert_eq!(incoming[0].from_ranges.len(), 1);

    let outgoing = analysis.outgoing_calls(&item);
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].to.name, "leaf");
    assert_eq!(outgoing[0].from_ranges.len(), 1);
}

#[test]
fn call_hierarchy_tracks_cross_file_imported_calls() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn shared() {
                        helper();
                    }

                    fn helper() {}
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        tools::shared();
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let analysis = host.snapshot();
    let provider = analysis
        .db
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let provider_text = analysis
        .file_text(provider)
        .expect("expected provider text");
    assert_no_syntax_diagnostics(&analysis, provider);

    let shared_offset =
        u32::try_from(provider_text.find("shared").expect("expected shared")).expect("offset");
    let item = analysis
        .prepare_call_hierarchy(FilePosition {
            file_id: provider,
            offset: shared_offset,
        })
        .into_iter()
        .find(|item| item.name == "shared")
        .expect("expected shared call hierarchy item");

    let incoming = analysis.incoming_calls(&item);
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].from.name, "run");

    let outgoing = analysis.outgoing_calls(&item);
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].to.name, "helper");
}
