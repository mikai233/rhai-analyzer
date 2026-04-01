use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::docs::builtin_type_docs;
use crate::builtin::signatures::helpers::builtin_documented_method;
use crate::types::{HostFunction, HostType};

const ARRAY_REFERENCE_URL: &str = "https://rhai.rs/book/ref/arrays.html";

fn array_method(
    name: &str,
    signatures: Vec<FunctionTypeRef>,
    summary: &str,
    examples: &[&str],
) -> HostFunction {
    builtin_documented_method(
        "array",
        name,
        signatures,
        summary,
        examples,
        ARRAY_REFERENCE_URL,
    )
}

pub(crate) fn builtin_array_type() -> HostType {
    HostType {
        name: "array".to_owned(),
        generic_params: Vec::new(),
        docs: Some(builtin_type_docs(
            "array",
            "Builtin Rhai array type for ordered collections and list-style mutation.",
            &[
                "let values = [1, 2, 3];",
                "values.push(4);",
                "// values == [1, 2, 3, 4]",
            ],
            ARRAY_REFERENCE_URL,
        )),
        methods: vec![
            array_method(
                "get",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                }],
                "Get a copy of the element at the specified position.",
                &[
                    "let values = [1, 2, 3];",
                    "let first = values.get(0);",
                    "// first == 1",
                ],
            ),
            array_method(
                "set",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int, TypeRef::Any],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Set the element at the specified position.",
                &[
                    "let values = [1, 2, 3];",
                    "values.set(1, 42);",
                    "// values == [1, 42, 3]",
                ],
            ),
            array_method(
                "push",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Any],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Append an element to the end of the array.",
                &[
                    "let values = [1, 2, 3];",
                    "values.push(4);",
                    "// values == [1, 2, 3, 4]",
                ],
            ),
            array_method(
                "append",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Array(Box::new(TypeRef::Any))],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Append all elements from another array.",
                &[
                    "let values = [1, 2];",
                    "values.append([3, 4]);",
                    "// values == [1, 2, 3, 4]",
                ],
            ),
            array_method(
                "insert",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int, TypeRef::Any],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Insert an element at the specified position.",
                &[
                    "let values = [1, 3];",
                    "values.insert(1, 2);",
                    "// values == [1, 2, 3]",
                ],
            ),
            array_method(
                "pop",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                }],
                "Remove the last element and return it.",
                &[
                    "let values = [1, 2, 3];",
                    "let last = values.pop();",
                    "// last == 3",
                    "// values == [1, 2]",
                ],
            ),
            array_method(
                "shift",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                }],
                "Remove the first element and return it.",
                &[
                    "let values = [1, 2, 3];",
                    "let first = values.shift();",
                    "// first == 1",
                    "// values == [2, 3]",
                ],
            ),
            array_method(
                "remove",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                }],
                "Remove the element at the specified position.",
                &[
                    "let values = [1, 2, 3];",
                    "let removed = values.remove(1);",
                    "// removed == 2",
                    "// values == [1, 3]",
                ],
            ),
            array_method(
                "reverse",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                "Reverse the array in place.",
                &[
                    "let values = [1, 2, 3];",
                    "values.reverse();",
                    "// values == [3, 2, 1]",
                ],
            ),
            array_method(
                "len",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                "Return the number of elements in the array.",
                &["let count = [1, 2, 3].len();", "// count == 3"],
            ),
            array_method(
                "is_empty",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if the array is empty.",
                &["let empty = [].is_empty();", "// empty == true"],
            ),
            array_method(
                "clear",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                "Clear the array in place.",
                &[
                    "let values = [1, 2, 3];",
                    "values.clear();",
                    "// values == []",
                ],
            ),
            array_method(
                "truncate",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Truncate the array at the specified length.",
                &[
                    "let values = [1, 2, 3, 4];",
                    "values.truncate(2);",
                    "// values == [1, 2]",
                ],
            ),
            array_method(
                "contains",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Any],
                    ret: Box::new(TypeRef::Bool),
                }],
                "Check whether the array contains a value.",
                &["let found = [1, 2, 3].contains(2);", "// found == true"],
            ),
            array_method(
                "index_of",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Any],
                        ret: Box::new(TypeRef::Int),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Any, TypeRef::Int],
                        ret: Box::new(TypeRef::Int),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::FnPtr],
                        ret: Box::new(TypeRef::Int),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::FnPtr, TypeRef::Int],
                        ret: Box::new(TypeRef::Int),
                    },
                ],
                "Find the position of a value, or of the first element that satisfies a predicate callback.",
                &["let index = [1, 2, 3].index_of(2);", "// index == 1"],
            ),
            array_method(
                "pad",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int, TypeRef::Any],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Extend the array to the target length by repeatedly appending a value.",
                &[
                    "let values = [1, 2];",
                    "values.pad(4, 0);",
                    "// values == [1, 2, 0, 0]",
                ],
            ),
            array_method(
                "extract",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int, TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Range],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::RangeInclusive],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                    },
                ],
                "Extract a portion of the array into a new array.",
                &[
                    "let values = [1, 2, 3, 4];",
                    "let part = values.extract(1..3);",
                    "// part == [2, 3]",
                ],
            ),
            array_method(
                "chop",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                }],
                "Cut elements from the front so the array is left at the target length.",
                &[
                    "let values = [1, 2, 3, 4];",
                    "let removed = values.chop(2);",
                    "// removed == [1, 2]",
                    "// values == [3, 4]",
                ],
            ),
            array_method(
                "drain",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::FnPtr],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int, TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Range],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::RangeInclusive],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                    },
                ],
                "Remove a range of elements and return them as a new array.",
                &[
                    "let values = [1, 2, 3, 4];",
                    "let removed = values.drain(1..3);",
                    "// removed == [2, 3]",
                    "// values == [1, 4]",
                ],
            ),
            array_method(
                "retain",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::FnPtr],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int, TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Range],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::RangeInclusive],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                    },
                ],
                "Retain selected elements in place and return the removed elements as a new array.",
                &[
                    "fn keep_even(x) { x % 2 == 0 }",
                    "let values = [1, 2, 3, 4];",
                    "let removed = values.retain(Fn(\"keep_even\"));",
                    "// removed == [1, 3]",
                    "// values == [2, 4]",
                ],
            ),
            array_method(
                "splice",
                vec![
                    FunctionTypeRef {
                        params: vec![
                            TypeRef::Int,
                            TypeRef::Int,
                            TypeRef::Array(Box::new(TypeRef::Any)),
                        ],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Range, TypeRef::Array(Box::new(TypeRef::Any))],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                    },
                    FunctionTypeRef {
                        params: vec![
                            TypeRef::RangeInclusive,
                            TypeRef::Array(Box::new(TypeRef::Any)),
                        ],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                    },
                ],
                "Replace a range with new elements and return the removed elements.",
                &[
                    "let values = [1, 2, 3, 4];",
                    "let removed = values.splice(1..3, [20, 30]);",
                    "// removed == [2, 3]",
                    "// values == [1, 20, 30, 4]",
                ],
            ),
            array_method(
                "dedup",
                vec![
                    FunctionTypeRef {
                        params: Vec::new(),
                        ret: Box::new(TypeRef::Unit),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::FnPtr],
                        ret: Box::new(TypeRef::Unit),
                    },
                ],
                "Remove consecutive duplicate elements in place.",
                &[
                    "let values = [1, 1, 2, 2, 3];",
                    "values.dedup();",
                    "// values == [1, 2, 3]",
                ],
            ),
            array_method(
                "sort",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                "Sort the array in ascending order in place.",
                &[
                    "let values = [3, 1, 2];",
                    "values.sort();",
                    "// values == [1, 2, 3]",
                ],
            ),
            array_method(
                "split",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::Array(Box::new(
                        TypeRef::Any,
                    ))))),
                }],
                "Split the array into two arrays at the specified position.",
                &[
                    "let parts = [1, 2, 3, 4].split(2);",
                    "// parts == [[1, 2], [3, 4]]",
                ],
            ),
            array_method(
                "for_each",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::FnPtr],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Run a callback for each element in order.",
                &[
                    "fn double_item() { this *= 2; }",
                    "let values = [1, 2, 3];",
                    "values.for_each(Fn(\"double_item\"));",
                    "// values == [2, 4, 6]",
                ],
            ),
            array_method(
                "filter",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::FnPtr],
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                }],
                "Return a new array containing only the elements accepted by the callback.",
                &[
                    "fn is_even(x) { x % 2 == 0 }",
                    "let values = [1, 2, 3, 4];",
                    "let evens = values.filter(Fn(\"is_even\"));",
                    "// evens == [2, 4]",
                ],
            ),
            array_method(
                "map",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::FnPtr],
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                }],
                "Return a new array containing the callback result for each element.",
                &[
                    "fn double(x) { x * 2 }",
                    "let values = [1, 2, 3];",
                    "let doubled = values.map(Fn(\"double\"));",
                    "// doubled == [2, 4, 6]",
                ],
            ),
            array_method(
                "some",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::FnPtr],
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if any element satisfies the predicate callback.",
                &[
                    "fn is_large(x) { x > 10 }",
                    "let values = [1, 20, 3];",
                    "let found = values.some(Fn(\"is_large\"));",
                    "// found == true",
                ],
            ),
            array_method(
                "all",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::FnPtr],
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if all elements satisfy the predicate callback.",
                &[
                    "fn is_positive(x) { x > 0 }",
                    "let values = [1, 2, 3];",
                    "let all_positive = values.all(Fn(\"is_positive\"));",
                    "// all_positive == true",
                ],
            ),
            array_method(
                "reduce",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::FnPtr],
                        ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::FnPtr, TypeRef::Any],
                        ret: Box::new(TypeRef::Any),
                    },
                ],
                "Reduce the array into a single value using a callback.",
                &[
                    "fn add(total, value) { total + value }",
                    "let values = [1, 2, 3];",
                    "let sum = values.reduce(Fn(\"add\"), 0);",
                    "// sum == 6",
                ],
            ),
            array_method(
                "reduce_rev",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::FnPtr],
                        ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::FnPtr, TypeRef::Any],
                        ret: Box::new(TypeRef::Any),
                    },
                ],
                "Reduce the array from the end toward the beginning using a callback.",
                &[
                    "fn append(acc, value) { acc + value }",
                    "let values = [\"a\", \"b\", \"c\"];",
                    "let text = values.reduce_rev(Fn(\"append\"), \"\");",
                    "// text == \"cba\"",
                ],
            ),
            array_method(
                "find",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::FnPtr],
                        ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::FnPtr, TypeRef::Int],
                        ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                    },
                ],
                "Return the first element that satisfies a predicate callback.",
                &[
                    "fn is_even(x) { x % 2 == 0 }",
                    "let value = [1, 3, 4, 7].find(Fn(\"is_even\"));",
                    "// value == 4",
                ],
            ),
            array_method(
                "find_map",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::FnPtr],
                        ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::FnPtr, TypeRef::Int],
                        ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                    },
                ],
                "Return the first non-unit value produced by the callback.",
                &[
                    "fn parse_even(x) { if x % 2 == 0 { x * 10 } else { () } }",
                    "let value = [1, 3, 4, 7].find_map(Fn(\"parse_even\"));",
                    "// value == 40",
                ],
            ),
            array_method(
                "zip",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Array(Box::new(TypeRef::Any)), TypeRef::FnPtr],
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                }],
                "Zip two arrays together and map each element pair through a callback.",
                &[
                    "fn add_pair(left, right) { left + right }",
                    "let values = [1, 2, 3].zip([10, 20, 30], Fn(\"add_pair\"));",
                    "// values == [11, 22, 33]",
                ],
            ),
            array_method(
                "sort_by",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::FnPtr],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Sort the array in place with a comparison callback.",
                &[
                    "fn descending(left, right) { right - left }",
                    "let values = [1, 3, 2];",
                    "values.sort_by(Fn(\"descending\"));",
                    "// values == [3, 2, 1]",
                ],
            ),
            array_method(
                "sort_desc",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                "Sort the array in descending order in place.",
                &[
                    "let values = [1, 3, 2];",
                    "values.sort_desc();",
                    "// values == [3, 2, 1]",
                ],
            ),
            array_method(
                "order_by",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::FnPtr],
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                }],
                "Return a sorted copy of the array using a comparison callback.",
                &[
                    "fn descending(left, right) { right - left }",
                    "let ordered = [1, 3, 2].order_by(Fn(\"descending\"));",
                    "// ordered == [3, 2, 1]",
                ],
            ),
            array_method(
                "order",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                }],
                "Return an ascending sorted copy of the array.",
                &[
                    "let ordered = [3, 1, 2].order();",
                    "// ordered == [1, 2, 3]",
                ],
            ),
            array_method(
                "order_desc",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                }],
                "Return a descending sorted copy of the array.",
                &[
                    "let ordered = [3, 1, 2].order_desc();",
                    "// ordered == [3, 2, 1]",
                ],
            ),
        ],
    }
}
