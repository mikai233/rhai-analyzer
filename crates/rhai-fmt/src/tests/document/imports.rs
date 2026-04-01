use crate::{FormatOptions, ImportSortOrder};

use crate::tests::{assert_formats_to, assert_formats_to_with_options};

#[test]
fn formatter_normalizes_import_export_sections() {
    let source = r#"import   "pkg"  as pkg;
import "tools";
export const CONFIG=#{name:"Ada",values:[1,2,3,4,5,6,7,8,9,10,11,12]};
export   helper   as public_helper;
fn run(){pkg::boot(); tools::boot();}
"#;

    let expected = r#"import "pkg" as pkg;
import "tools";

export const CONFIG = #{
    name: "Ada",
    values: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
};
export helper as public_helper;

fn run() {
    pkg::boot();
    tools::boot();
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_can_sort_top_level_import_runs_by_module_path() {
    let source = r#"import "zebra" as zebra;
import "alpha";
import "beta" as beta;
fn run(){}
"#;
    let expected = r#"import "alpha";
import "beta" as beta;
import "zebra" as zebra;

fn run() {}
"#;

    assert_formats_to_with_options(
        source,
        expected,
        &FormatOptions {
            import_sort_order: ImportSortOrder::ModulePath,
            ..FormatOptions::default()
        },
    );
}
#[test]
fn formatter_preserves_blank_line_separated_import_groups_when_sorting() {
    let source = r#"import "zebra";
import "alpha";

import "delta";
import "beta";
fn run(){}
"#;
    let expected = r#"import "alpha";
import "zebra";

import "beta";
import "delta";

fn run() {}
"#;

    assert_formats_to_with_options(
        source,
        expected,
        &FormatOptions {
            import_sort_order: ImportSortOrder::ModulePath,
            ..FormatOptions::default()
        },
    );
}
#[test]
fn formatter_does_not_reorder_skipped_imports() {
    let source = r#"// rhai-fmt: skip
import "zebra" as zebra;
import "alpha";
fn run(){}
"#;

    let expected = r#"// rhai-fmt: skip
import "zebra" as zebra;
import "alpha";

fn run() {}
"#;

    assert_formats_to_with_options(
        source,
        expected,
        &FormatOptions {
            import_sort_order: ImportSortOrder::ModulePath,
            ..FormatOptions::default()
        },
    );
}
#[test]
fn formatter_wraps_long_import_export_heads_under_width_constraints() {
    let source = r#"
import "very_long_module_path_name" as profile_name;
export helper_with_a_really_long_public_name as public_name;
"#;

    let expected = r#"import
    "very_long_module_path_name" as profile_name;

export
    helper_with_a_really_long_public_name as public_name;
"#;

    assert_formats_to_with_options(
        source,
        expected,
        &FormatOptions {
            max_line_length: 40,
            ..FormatOptions::default()
        },
    );
}
