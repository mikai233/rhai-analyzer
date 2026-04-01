use std::path::Path;

use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, offset_in};
use crate::{AnalyzerDatabase, ChangeSet, ProjectReferenceKind};
use rhai_syntax::TextSize;
use rhai_vfs::DocumentVersion;

#[test]
fn goto_definition_resolves_outer_scope_captures_inside_functions() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            export const DEFAULTS = #{ name: "demo" };

            fn make_config() {
                let config = #{
                    defaults: DEFAULTS,
                };
                config
            }
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = snapshot.file_text(file_id).expect("expected file text");

    let usage_offset = offset_in(&text, "defaults: DEFAULTS") + TextSize::from(10);
    let declaration_offset = offset_in(&text, "DEFAULTS =");
    let definitions = snapshot.goto_definition(file_id, usage_offset);

    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].file_id, file_id);
    assert!(
        definitions[0]
            .target
            .full_range
            .contains(declaration_offset),
        "expected goto definition to target outer captured const, got {definitions:?}"
    );
}
#[test]
fn goto_definition_prefers_local_overload_matching_argument_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @param value int
            fn do_something(value) {}

            /// @param value string
            fn do_something(value) {}

            do_something("hello");
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let text = snapshot.file_text(file_id).expect("expected text");
    let usage_offset = offset_in(&text, "do_something(\"hello\")") + TextSize::from(1);
    let string_overload_offset = text
        .match_indices("fn do_something")
        .nth(1)
        .map(|(offset, _)| TextSize::from(offset as u32 + 3))
        .expect("expected second function declaration");

    let definitions = snapshot.goto_definition(file_id, usage_offset);

    assert_eq!(definitions.len(), 1, "{definitions:?}");
    assert_eq!(definitions[0].file_id, file_id);
    assert!(
        definitions[0]
            .target
            .full_range
            .contains(string_overload_offset),
        "expected string overload definition, got {definitions:?}"
    );
}
#[test]
fn navigation_resolves_param_and_local_refs_inside_object_field_values() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn make_config(root, mode) {
                let workspace_name = workspace::name(root);
                let config = #{
                    mode: mode,
                    workspace: workspace_name,
                };
                config
            }
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = snapshot.file_text(file_id).expect("expected file text");

    let mode_decl_offset = offset_in(&text, "root, mode") + TextSize::from(6);
    let mode_usage_offset = offset_in(&text, "mode: mode") + TextSize::from(7);
    let mode_definitions = snapshot.goto_definition(file_id, mode_usage_offset);
    assert_eq!(mode_definitions.len(), 1);
    assert!(
        mode_definitions[0]
            .target
            .full_range
            .contains(mode_decl_offset),
        "expected mode usage to target parameter definition, got {mode_definitions:?}"
    );

    let workspace_decl_offset = offset_in(&text, "workspace_name =");
    let workspace_usage_offset = offset_in(&text, "workspace: workspace_name") + TextSize::from(11);
    let workspace_definitions = snapshot.goto_definition(file_id, workspace_usage_offset);
    assert_eq!(workspace_definitions.len(), 1);
    assert!(
        workspace_definitions[0]
            .target
            .full_range
            .contains(workspace_decl_offset),
        "expected workspace_name usage to target local definition, got {workspace_definitions:?}"
    );

    let mode_references = snapshot
        .find_references(file_id, mode_decl_offset)
        .expect("expected mode references");
    assert!(mode_references.references.iter().any(|reference| {
        reference.file_id == file_id
            && reference.kind == ProjectReferenceKind::Reference
            && reference.range.contains(mode_usage_offset)
    }));

    let workspace_references = snapshot
        .find_references(file_id, workspace_decl_offset)
        .expect("expected workspace_name references");
    assert!(workspace_references.references.iter().any(|reference| {
        reference.file_id == file_id
            && reference.kind == ProjectReferenceKind::Reference
            && reference.range.contains(workspace_usage_offset)
    }));
}
#[test]
fn goto_definition_resolves_object_field_member_access_to_field_declaration() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            export const DEFAULTS = #{
                name: "demo",
                watch: true,
            };

            let value = DEFAULTS.name;
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = snapshot.file_text(file_id).expect("expected file text");

    let declaration_offset = offset_in(&text, "name: \"demo\"");
    let usage_offset = offset_in(&text, "DEFAULTS.name") + TextSize::from(9);
    let definitions = snapshot.goto_definition(file_id, usage_offset);

    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].file_id, file_id);
    assert!(
        definitions[0]
            .target
            .full_range
            .contains(declaration_offset),
        "expected field member access to resolve to object field declaration, got {definitions:?}"
    );
}
