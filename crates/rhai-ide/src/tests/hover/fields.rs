use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, FilePosition, HoverSignatureSource};

#[test]
fn hover_reports_inferred_types_for_object_field_member_accesses() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let defaults = #{
                name: "demo",
                watch: true,
            };

            let value = defaults.name;
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
    let offset = u32::try_from(text.find("defaults.name").expect("expected field usage") + 9)
        .expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected field hover");

    assert_eq!(hover.signature, "field name: string");
    assert_eq!(hover.source, HoverSignatureSource::Inferred);
    assert!(hover.declared_signature.is_none());
    assert_eq!(
        hover.inferred_signature.as_deref(),
        Some("field name: string")
    );
    assert!(hover.notes.iter().any(|note| {
        note == "Field type is inferred from structural object flows and object literal analysis."
    }));
}
#[test]
fn hover_prefers_documented_object_field_annotations() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @field name string
            let user = #{
                name: "Ada",
            };

            let value = user.name;
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
    let offset =
        u32::try_from(text.find("user.name").expect("expected field usage") + 5).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected documented field hover");

    assert_eq!(hover.signature, "field name: string");
    assert_eq!(hover.source, HoverSignatureSource::Declared);
    assert_eq!(
        hover.declared_signature.as_deref(),
        Some("field name: string")
    );
    assert_eq!(
        hover.inferred_signature.as_deref(),
        Some("field name: string")
    );
}
#[test]
fn hover_surfaces_documented_object_field_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @field name string Primary display name
            let user = #{
                name: "Ada",
            };

            let value = user.name;
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
    let offset =
        u32::try_from(text.find("user.name").expect("expected field usage") + 5).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected documented field hover");

    assert_eq!(hover.signature, "field name: string");
    assert_eq!(hover.docs.as_deref(), Some("Primary display name"));
}
