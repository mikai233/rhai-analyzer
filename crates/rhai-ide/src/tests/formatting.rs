use crate::AnalysisHost;
use crate::tests::assert_no_syntax_diagnostics;
use rhai_db::ChangeSet;
use rhai_fmt::{ContainerLayoutStyle, FormatOptions, ImportSortOrder, IndentStyle};
use rhai_syntax::TextRange;
use rhai_vfs::DocumentVersion;

#[test]
fn format_document_returns_whole_file_rewrite_when_formatter_changes_text() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "fn run(){let value=1+2;value}\n",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .file_id_for_path(std::path::Path::new("main.rhai"))
        .expect("expected file id");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let change = analysis
        .format_document(file_id)
        .expect("expected formatting change");
    let [file_edit] = change.file_edits.as_slice() else {
        panic!("expected one file edit");
    };
    let [edit] = file_edit.edits.as_slice() else {
        panic!("expected one text edit");
    };

    assert_eq!(edit.range.start(), 0.into());
    assert_eq!(
        edit.new_text,
        "fn run() {\n    let value = 1 + 2;\n    value\n}\n"
    );
}

#[test]
fn format_document_returns_none_when_text_is_already_stable() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "fn run() {\n    let value = 1 + 2;\n    value\n}\n",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .file_id_for_path(std::path::Path::new("main.rhai"))
        .expect("expected file id");
    assert_no_syntax_diagnostics(&analysis, file_id);

    assert!(analysis.format_document(file_id).is_none());
}

#[test]
fn format_range_returns_partial_edit_when_selection_intersects_changed_region() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let prefix = 1;\nfn run(){let value=1+2;value}\n",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .file_id_for_path(std::path::Path::new("main.rhai"))
        .expect("expected file id");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let start = "let prefix = 1;\n".len() as u32;
    let end = start + "fn run(){let value=1+2;value}\n".len() as u32;
    let change = analysis
        .format_range(file_id, TextRange::new(start.into(), end.into()))
        .expect("expected range formatting change");
    let [file_edit] = change.file_edits.as_slice() else {
        panic!("expected one file edit");
    };
    let [edit] = file_edit.edits.as_slice() else {
        panic!("expected one text edit");
    };

    assert_eq!(u32::from(edit.range.start()), start);
    assert!(u32::from(edit.range.end()) <= end);
    assert!(edit.new_text.contains("fn run"));
    assert!(edit.new_text.contains("let value = 1 + 2;"));
}

#[test]
fn format_document_accepts_explicit_formatter_options() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "fn run(){let values=[12345,67890,abcde];}\n",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .file_id_for_path(std::path::Path::new("main.rhai"))
        .expect("expected file id");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let change = analysis
        .format_document_with_options(
            file_id,
            &FormatOptions {
                indent_style: IndentStyle::Tabs,
                indent_width: 2,
                max_line_length: 12,
                trailing_commas: false,
                final_newline: false,
                ..FormatOptions::default()
            },
        )
        .expect("expected formatting change");
    let [file_edit] = change.file_edits.as_slice() else {
        panic!("expected one file edit");
    };
    let [edit] = file_edit.edits.as_slice() else {
        panic!("expected one text edit");
    };

    assert!(edit.new_text.contains("\tlet values = ["));
    assert!(edit.new_text.contains("\t\t12345,"));
    assert!(edit.new_text.contains("\t\tabcde\n\t];"));
    assert!(!edit.new_text.contains("\t\tabcde,\n\t];"));
    assert!(!edit.new_text.ends_with('\n'));
}

#[test]
fn format_document_accepts_container_layout_preferences() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "fn run(){let values=[1,2,3]; helper(alpha,beta);}\n",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .file_id_for_path(std::path::Path::new("main.rhai"))
        .expect("expected file id");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let change = analysis
        .format_document_with_options(
            file_id,
            &FormatOptions {
                container_layout: ContainerLayoutStyle::PreferMultiLine,
                ..FormatOptions::default()
            },
        )
        .expect("expected formatting change");
    let [file_edit] = change.file_edits.as_slice() else {
        panic!("expected one file edit");
    };
    let [edit] = file_edit.edits.as_slice() else {
        panic!("expected one text edit");
    };

    assert!(edit.new_text.contains("let values = [\n"));
    assert!(edit.new_text.contains("helper(\n"));
}

#[test]
fn format_document_accepts_import_sorting_preferences() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "import \"zebra\" as zebra;\nimport \"alpha\";\nimport \"beta\" as beta;\nfn run(){}\n",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .file_id_for_path(std::path::Path::new("main.rhai"))
        .expect("expected file id");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let change = analysis
        .format_document_with_options(
            file_id,
            &FormatOptions {
                import_sort_order: ImportSortOrder::ModulePath,
                ..FormatOptions::default()
            },
        )
        .expect("expected formatting change");
    let [file_edit] = change.file_edits.as_slice() else {
        panic!("expected one file edit");
    };
    let [edit] = file_edit.edits.as_slice() else {
        panic!("expected one text edit");
    };

    assert!(
        edit.new_text.starts_with(
            "import \"alpha\";\nimport \"beta\" as beta;\nimport \"zebra\" as zebra;\n"
        )
    );
}
