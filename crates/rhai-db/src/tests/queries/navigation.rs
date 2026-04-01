use std::path::Path;

use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, offset_in};
use crate::{AnalyzerDatabase, ChangeSet, FileChange, ProjectDiagnosticCode, ProjectReferenceKind};
use rhai_hir::SemanticDiagnosticCode;
use rhai_project::ProjectConfig;
use rhai_syntax::TextSize;
use rhai_vfs::DocumentVersion;

#[test]
fn imported_member_references_report_unresolved_after_provider_renames() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() { 1 }
                    export const VALUE = 1;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;
                    fn run() {
                        tools::helper();
                        let value = tools::VALUE;
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(ProjectConfig::default()),
    });

    db.apply_change(ChangeSet::single_file(
        "provider.rhai",
        r#"
            fn renamed_helper() { 1 }
            export const RENAMED = 1;
        "#,
        DocumentVersion(2),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");

    let diagnostics = snapshot.project_diagnostics(consumer);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == ProjectDiagnosticCode::UnresolvedImportMember)
    );
    assert!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == ProjectDiagnosticCode::UnresolvedImportMember)
            .count()
            >= 2
    );
}
#[test]
fn static_path_imports_report_missing_workspace_files() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            import "./missing_module" as missing;
            fn run() {}
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");

    let diagnostics = snapshot.project_diagnostics(file_id);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == ProjectDiagnosticCode::BrokenLinkedImport)
    );
}
#[test]
fn static_named_imports_report_unresolved_modules() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            import "env" as env;
            fn run() {}
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");

    let diagnostics = snapshot.project_diagnostics(file_id);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code
            == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedImportModule)
    }));
}
#[test]
fn changing_exports_does_not_break_static_import_linkage() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    let helper = 1;
                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"import "provider" as tools;"#.to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let consumer = first_snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_eq!(first_snapshot.linked_imports(consumer).len(), 1);

    db.apply_change(ChangeSet::single_file(
        "provider.rhai",
        r#"
            let helper = 1;
            export helper as renamed_tools;
        "#,
        DocumentVersion(2),
    ));

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    assert_eq!(second_snapshot.linked_imports(consumer).len(), 1);
    assert!(second_snapshot.exports_named("shared_tools").is_empty());
    assert_eq!(second_snapshot.exports_named("renamed_tools").len(), 1);
}
#[test]
fn change_report_surfaces_dependency_affected_files_for_static_imports() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: "export const VALUE = 1;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"import "provider" as tools;"#.to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let impact = db.apply_change_report(ChangeSet::single_file(
        "provider.rhai",
        "export const VALUE = 2;",
        DocumentVersion(2),
    ));

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

    assert_eq!(impact.changed_files, vec![provider]);
    assert_eq!(impact.rebuilt_files, vec![provider]);
    assert_eq!(impact.dependency_affected_files, vec![consumer]);
}
#[test]
fn change_report_marks_dependencies_affected_when_importers_change() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: "export const VALUE = 1;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"import "provider" as tools;"#.to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let impact = db.apply_change_report(ChangeSet::single_file(
        "consumer.rhai",
        "import \"provider\" as tools;\nfn run() {}",
        DocumentVersion(2),
    ));

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

    assert_eq!(impact.changed_files, vec![consumer]);
    assert_eq!(impact.rebuilt_files, vec![consumer]);
    assert_eq!(impact.dependency_affected_files, vec![provider]);
}
#[test]
fn project_find_references_for_exports_stay_local_to_the_exporting_file() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    let helper = 1;
                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let provider_text = snapshot
        .file_text(provider)
        .expect("expected provider text");

    let references = snapshot
        .find_references(provider, offset_in(&provider_text, "shared_tools"))
        .expect("expected project references");

    assert_eq!(references.targets.len(), 1);
    assert_eq!(references.targets[0].symbol.name, "shared_tools");
    assert_eq!(
        references
            .references
            .iter()
            .map(|reference| (reference.file_id, reference.kind))
            .collect::<Vec<_>>(),
        vec![(provider, ProjectReferenceKind::Definition)]
    );
}
#[test]
fn goto_definition_on_import_module_reference_targets_provider_file() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
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
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer_text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");
    let module_offset = offset_in(&consumer_text, "\"provider\"") + TextSize::from(1);

    let definitions = snapshot.goto_definition(consumer, module_offset);

    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].file_id, provider);
}
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
fn find_references_on_import_alias_reports_current_file_usages() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() {
                        1
                    }

                    export const VALUE = 1;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        tools::helper();
                        let value = tools::VALUE;
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let consumer_text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");
    let alias_offset = offset_in(&consumer_text, "tools");

    let references = snapshot
        .find_references(consumer, alias_offset)
        .expect("expected project references");

    assert_eq!(references.targets.len(), 1);
    assert_eq!(references.targets[0].symbol.name, "tools");
    assert_eq!(references.targets[0].file_id, consumer);
    assert_eq!(references.references.len(), 3);
    assert_eq!(
        references
            .references
            .iter()
            .map(|reference| (reference.file_id, reference.kind))
            .collect::<Vec<_>>(),
        vec![
            (consumer, ProjectReferenceKind::Definition),
            (consumer, ProjectReferenceKind::Reference),
            (consumer, ProjectReferenceKind::Reference),
        ]
    );
}
#[test]
fn find_references_on_imported_path_member_reaches_provider_symbol() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
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
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        tools::helper();
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
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

    let references = snapshot
        .find_references(provider, offset_in(&provider_text, "helper"))
        .expect("expected project references");

    assert_eq!(references.targets.len(), 1);
    assert_eq!(references.targets[0].symbol.name, "helper");
    assert!(
        references
            .references
            .iter()
            .any(|reference| reference.file_id == consumer
                && reference.kind == ProjectReferenceKind::Reference),
        "expected imported path reference, got {:?}",
        references.references
    );
}
#[test]
fn find_references_include_outer_scope_captures_inside_functions() {
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

    let declaration_offset = offset_in(&text, "DEFAULTS =");
    let usage_offset = offset_in(&text, "defaults: DEFAULTS") + TextSize::from(10);
    let references = snapshot
        .find_references(file_id, declaration_offset)
        .expect("expected project references");

    assert_eq!(references.targets.len(), 1);
    assert!(references.references.iter().any(|reference| {
        reference.file_id == file_id
            && reference.kind == ProjectReferenceKind::Reference
            && reference.range.contains(usage_offset)
    }));
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
#[test]
fn project_rename_plan_for_exports_does_not_include_module_import_paths() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    let helper = 1;
                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let provider_text = snapshot
        .file_text(provider)
        .expect("expected provider text");

    let export_plan = snapshot
        .rename_plan(
            provider,
            offset_in(&provider_text, "shared_tools"),
            "renamed_tools",
        )
        .expect("expected project rename plan");
    assert_eq!(export_plan.targets.len(), 1);
    assert_eq!(export_plan.targets[0].symbol.name, "shared_tools");
    assert_eq!(export_plan.occurrences.len(), 1);
    assert_eq!(export_plan.occurrences[0].file_id, provider);
    assert_eq!(
        export_plan.occurrences[0].kind,
        ProjectReferenceKind::Definition
    );
}
