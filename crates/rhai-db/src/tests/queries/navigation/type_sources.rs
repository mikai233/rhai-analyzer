use std::path::Path;

use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, offset_in};
use crate::{AnalyzerDatabase, ChangeSet, FileChange, ProjectReferenceKind};
use rhai_project::ProjectConfig;
use rhai_syntax::TextSize;
use rhai_vfs::DocumentVersion;

#[test]
fn goto_type_definition_traces_structural_object_sources_through_symbol_flows() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let original = #{
                name: "demo",
                watch: true,
            };
            let alias = original;
            let current = alias;
            current.name
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let text = snapshot.file_text(file_id).expect("expected file text");

    let usage_offset = offset_in(&text, "current.name") + TextSize::from(1);
    let literal_offset = offset_in(&text, "#{") + TextSize::from(1);
    let definitions = snapshot.goto_type_definition(file_id, usage_offset);

    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].file_id, file_id);
    assert!(
        definitions[0].target.full_range.contains(literal_offset),
        "expected type definition to target structural object source, got {definitions:?}"
    );
}
#[test]
fn goto_type_definition_can_target_documented_symbol_annotations() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @type int
            let answer = 1;
            answer
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let text = snapshot.file_text(file_id).expect("expected file text");

    let usage_offset = offset_in(&text, "answer\n") + TextSize::from(1);
    let docs_offset = offset_in(&text, "@type int") + TextSize::from(1);
    let definitions = snapshot.goto_type_definition(file_id, usage_offset);

    assert_eq!(definitions.len(), 1);
    assert!(
        definitions[0].target.full_range.contains(docs_offset),
        "expected type definition to target doc annotation block, got {definitions:?}"
    );
}
#[test]
fn find_references_for_object_field_declaration_include_cross_file_member_accesses() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    export const DEFAULTS = #{
                        name: "demo",
                        watch: true,
                    };
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    let value = tools::DEFAULTS.name;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(ProjectConfig::default()),
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let provider_text = snapshot
        .file_text(provider)
        .expect("expected provider text");
    let consumer_text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");

    let declaration_offset = offset_in(&provider_text, "name: \"demo\"");
    let usage_offset = offset_in(&consumer_text, "DEFAULTS.name") + TextSize::from(9);
    let references = snapshot
        .find_references(provider, declaration_offset)
        .expect("expected object field references");

    assert!(references.references.iter().any(|reference| {
        reference.file_id == provider && reference.kind == ProjectReferenceKind::Definition
    }));
    assert!(references.references.iter().any(|reference| {
        reference.file_id == consumer
            && reference.kind == ProjectReferenceKind::Reference
            && reference.range.contains(usage_offset)
    }));
}
