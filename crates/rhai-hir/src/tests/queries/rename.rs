use crate::tests::parse_valid;
use crate::{SymbolKind, lower_file};
use rhai_syntax::TextSize;

#[test]
fn editable_rename_occurrence_classifies_definitions_and_references_only() {
    let source = r#"
            fn helper() {}
            let value = helper();
            let text = "helper";
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    let def_offset = TextSize::from(u32::try_from(source.find("helper").unwrap()).unwrap());
    let ref_offset = TextSize::from(u32::try_from(source.find("helper();").unwrap()).unwrap());
    let string_offset =
        TextSize::from(u32::try_from(source.find("\"helper\"").unwrap() + 1).unwrap());

    let def_occurrence = hir
        .editable_rename_occurrence_at(def_offset)
        .expect("expected definition occurrence");
    let ref_occurrence = hir
        .editable_rename_occurrence_at(ref_offset)
        .expect("expected reference occurrence");

    assert_eq!(def_occurrence.kind, crate::RenameOccurrenceKind::Definition);
    assert_eq!(ref_occurrence.kind, crate::RenameOccurrenceKind::Reference);
    assert!(hir.editable_rename_occurrence_at(string_offset).is_none());
}
#[test]
fn rename_plan_tracks_occurrences_aliases_and_preflight_issues() {
    let parse = parse_valid(
        r#"
            let taken = 0;
            let helper = 1;
            import helper as helper_alias;
            export helper as public_api;

            {
                let public_helper = 1;
                helper;
            }
        "#,
    );
    let hir = lower_file(&parse);

    let helper = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "helper" && symbol.kind == SymbolKind::Variable)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `helper` symbol");

    let plan = hir.rename_plan(helper, "public_helper");

    assert_eq!(plan.target.name, "helper");
    assert_eq!(plan.new_name, "public_helper");
    assert_eq!(
        plan.occurrences
            .iter()
            .filter(|occurrence| occurrence.kind == crate::RenameOccurrenceKind::Definition)
            .count(),
        1
    );
    assert_eq!(
        plan.occurrences
            .iter()
            .filter(|occurrence| occurrence.kind == crate::RenameOccurrenceKind::Reference)
            .count(),
        3
    );
    assert_eq!(plan.linked_aliases.len(), 2);
    assert!(plan.linked_aliases.iter().any(|alias| {
        alias.kind == crate::LinkedAliasKind::ImportAlias && alias.symbol.name == "helper_alias"
    }));
    assert!(plan.linked_aliases.iter().any(|alias| {
        alias.kind == crate::LinkedAliasKind::ExportAlias && alias.symbol.name == "public_api"
    }));
    assert!(plan.issues.iter().any(|issue| {
        issue.kind == crate::RenamePreflightIssueKind::ReferenceCollision
            && issue
                .related_symbol
                .as_ref()
                .map(|symbol| symbol.name.as_str())
                == Some("public_helper")
    }));
    assert!(
        !plan
            .issues
            .iter()
            .any(|issue| issue.kind == crate::RenamePreflightIssueKind::DuplicateDefinition)
    );
}
#[test]
fn rename_preflight_reports_duplicate_definitions_in_same_scope() {
    let parse = parse_valid(
        r#"
            let taken = 1;
            let value = 2;
        "#,
    );
    let hir = lower_file(&parse);

    let value = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "value" && symbol.kind == SymbolKind::Variable)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `value` symbol");

    let plan = hir.rename_plan(value, "taken");
    assert!(plan.issues.iter().any(|issue| {
        issue.kind == crate::RenamePreflightIssueKind::DuplicateDefinition
            && issue
                .related_symbol
                .as_ref()
                .map(|symbol| symbol.name.as_str())
                == Some("taken")
    }));
}
