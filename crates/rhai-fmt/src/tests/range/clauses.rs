use crate::FormatOptions;
use crate::tests::range::{assert_range_rewrites_to, default_range};

#[test]
fn range_formatter_can_target_for_bindings() {
    let source = "fn run(){for (item,index) in values {index}}\n";
    let selection_start =
        u32::try_from(source.find("item").expect("expected bindings")).expect("offset");
    let selection_end =
        u32::try_from(source.find(") in").expect("expected binding end") + 1).expect("offset");

    assert_range_rewrites_to(
        source,
        default_range(selection_start, selection_end),
        &FormatOptions::default(),
        "fn run(){for (item, index) in values {index}}\n",
    );
}

#[test]
fn range_formatter_can_target_do_conditions() {
    let source = "fn run(){do { work() } while ready&&steady}\n";
    let selection_start =
        u32::try_from(source.find("while").expect("expected do condition")).expect("offset");
    let selection_end = u32::try_from(source.len() - 2).expect("offset");

    assert_range_rewrites_to(
        source,
        default_range(selection_start, selection_end),
        &FormatOptions::default(),
        "fn run(){do { work() } while ready && steady}\n",
    );
}

#[test]
fn range_formatter_can_target_catch_clauses() {
    let source = "fn run(){try { work() } catch (err){handle(err+1)}}\n";
    let selection_start =
        u32::try_from(source.find("catch").expect("expected catch clause")).expect("offset");
    let selection_end = u32::try_from(source.len() - 2).expect("offset");

    assert_range_rewrites_to(
        source,
        default_range(selection_start, selection_end),
        &FormatOptions::default(),
        "fn run(){try { work() } catch (err) {\n        handle(err + 1)\n    }}\n",
    );
}

#[test]
fn range_formatter_can_target_alias_clauses() {
    let source = "import \"pkg\" as   helper;\n";
    let selection_start =
        u32::try_from(source.find("as").expect("expected alias clause")).expect("offset");
    let selection_end =
        u32::try_from(source.find(";").expect("expected alias end")).expect("offset");

    assert_range_rewrites_to(
        source,
        default_range(selection_start, selection_end),
        &FormatOptions::default(),
        "import \"pkg\" as helper;\n",
    );
}

#[test]
fn range_formatter_can_target_else_branches() {
    let source = "fn run(){if ready { work() } else { fallback(1+2) }}\n";
    let selection_start =
        u32::try_from(source.find("else").expect("expected else branch")).expect("offset");
    let selection_end = u32::try_from(source.len() - 2).expect("offset");

    assert_range_rewrites_to(
        source,
        default_range(selection_start, selection_end),
        &FormatOptions::default(),
        "fn run(){if ready { work() } else {\n        fallback(1 + 2)\n    }}\n",
    );
}
