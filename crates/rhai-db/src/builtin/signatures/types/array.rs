use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::docs::{
    BuiltinCallableOverloadDoc, BuiltinParamDoc, builtin_type_docs,
};
use crate::builtin::signatures::helpers::{
    builtin_documented_method, builtin_documented_overloaded_method,
};
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

fn array_overloaded_method(
    name: &str,
    summary: &str,
    overloads: Vec<BuiltinCallableOverloadDoc<'_>>,
) -> HostFunction {
    builtin_documented_overloaded_method("array", name, summary, overloads, ARRAY_REFERENCE_URL)
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
            array_overloaded_method(
                "index_of",
                "Find the position of a value, or of the first element that satisfies a predicate callback.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Any],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Return the index of the first element equal to the requested value, or `-1` when it is absent.",
                        params: &[BuiltinParamDoc {
                            name: "needle",
                            description: "Value to search for using Rhai equality semantics.",
                        }],
                        examples: &["let index = [1, 2, 3].index_of(2);", "// index == 1"],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Any, TypeRef::Int],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Return the index of the first matching value, starting from a given array offset.",
                        params: &[
                            BuiltinParamDoc {
                                name: "needle",
                                description: "Value to search for using Rhai equality semantics.",
                            },
                            BuiltinParamDoc {
                                name: "start",
                                description: "Array index where the search begins.",
                            },
                        ],
                        examples: &["let index = [1, 2, 3, 2].index_of(2, 2);", "// index == 3"],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::FnPtr],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Return the index of the first element accepted by a predicate callback, or `-1` when none match.",
                        params: &[BuiltinParamDoc {
                            name: "predicate",
                            description: "Function pointer called for each element until it returns `true`.",
                        }],
                        examples: &[
                            "fn is_even(value) { value % 2 == 0 }",
                            "let index = [1, 3, 4, 7].index_of(Fn(\"is_even\"));",
                            "// index == 2",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::FnPtr, TypeRef::Int],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Return the index of the first element accepted by a predicate callback, starting from a given array offset.",
                        params: &[
                            BuiltinParamDoc {
                                name: "predicate",
                                description: "Function pointer called for each element until it returns `true`.",
                            },
                            BuiltinParamDoc {
                                name: "start",
                                description: "Array index where the search begins.",
                            },
                        ],
                        examples: &[
                            "fn is_even(value) { value % 2 == 0 }",
                            "let index = [2, 4, 6].index_of(Fn(\"is_even\"), 1);",
                            "// index == 1",
                        ],
                    },
                ],
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
            array_overloaded_method(
                "extract",
                "Extract a portion of the array into a new array.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                        },
                        summary: "Return the suffix starting at the requested array index.",
                        params: &[BuiltinParamDoc {
                            name: "start",
                            description: "Array index where the extracted suffix begins.",
                        }],
                        examples: &[
                            "let values = [1, 2, 3, 4];",
                            "let part = values.extract(2);",
                            "// part == [3, 4]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                        },
                        summary: "Return a slice using a starting index and an explicit element count.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Array index where extraction begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of elements to copy into the new array.",
                            },
                        ],
                        examples: &[
                            "let values = [1, 2, 3, 4];",
                            "let part = values.extract(1, 2);",
                            "// part == [2, 3]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                        },
                        summary: "Return a slice selected by an exclusive array range.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Exclusive range of element indexes to copy.",
                        }],
                        examples: &[
                            "let values = [1, 2, 3, 4];",
                            "let part = values.extract(1..3);",
                            "// part == [2, 3]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                        },
                        summary: "Return a slice selected by an inclusive array range.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Inclusive range of element indexes to copy.",
                        }],
                        examples: &[
                            "let values = [1, 2, 3, 4];",
                            "let part = values.extract(1..=2);",
                            "// part == [2, 3]",
                        ],
                    },
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
            array_overloaded_method(
                "drain",
                "Remove a range of elements and return them as a new array.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::FnPtr],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                        },
                        summary: "Remove every element accepted by a predicate callback and return the removed elements as a new array.",
                        params: &[BuiltinParamDoc {
                            name: "predicate",
                            description: "Function pointer called for each element; matching elements are removed.",
                        }],
                        examples: &[
                            "fn is_even(value) { value % 2 == 0 }",
                            "let values = [1, 2, 3, 4];",
                            "let removed = values.drain(Fn(\"is_even\"));",
                            "// removed == [2, 4]",
                            "// values == [1, 3]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                        },
                        summary: "Remove a fixed number of elements starting at an array index and return them as a new array.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Array index where removal begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of elements to remove.",
                            },
                        ],
                        examples: &[
                            "let values = [1, 2, 3, 4];",
                            "let removed = values.drain(1, 2);",
                            "// removed == [2, 3]",
                            "// values == [1, 4]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                        },
                        summary: "Remove an exclusive range of elements and return them as a new array.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Exclusive range of element indexes to remove.",
                        }],
                        examples: &[
                            "let values = [1, 2, 3, 4];",
                            "let removed = values.drain(1..3);",
                            "// removed == [2, 3]",
                            "// values == [1, 4]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                        },
                        summary: "Remove an inclusive range of elements and return them as a new array.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Inclusive range of element indexes to remove.",
                        }],
                        examples: &[
                            "let values = [1, 2, 3, 4];",
                            "let removed = values.drain(1..=2);",
                            "// removed == [2, 3]",
                            "// values == [1, 4]",
                        ],
                    },
                ],
            ),
            array_overloaded_method(
                "retain",
                "Retain selected elements in place and return the removed elements as a new array.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::FnPtr],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                        },
                        summary: "Keep only the elements accepted by a predicate callback and return the removed elements as a new array.",
                        params: &[BuiltinParamDoc {
                            name: "predicate",
                            description: "Function pointer called for each element; matching elements stay in the array.",
                        }],
                        examples: &[
                            "fn keep_even(x) { x % 2 == 0 }",
                            "let values = [1, 2, 3, 4];",
                            "let removed = values.retain(Fn(\"keep_even\"));",
                            "// removed == [1, 3]",
                            "// values == [2, 4]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                        },
                        summary: "Keep a contiguous array window in place and return all removed elements as a new array.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Array index where the retained window begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of elements to keep.",
                            },
                        ],
                        examples: &[
                            "let values = [1, 2, 3, 4];",
                            "let removed = values.retain(1, 2);",
                            "// removed == [1, 4]",
                            "// values == [2, 3]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                        },
                        summary: "Keep an exclusive range of elements in place and return the removed elements as a new array.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Exclusive range of element indexes to keep.",
                        }],
                        examples: &[
                            "let values = [1, 2, 3, 4];",
                            "let removed = values.retain(1..3);",
                            "// removed == [1, 4]",
                            "// values == [2, 3]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                        },
                        summary: "Keep an inclusive range of elements in place and return the removed elements as a new array.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Inclusive range of element indexes to keep.",
                        }],
                        examples: &[
                            "let values = [1, 2, 3, 4];",
                            "let removed = values.retain(1..=2);",
                            "// removed == [1, 4]",
                            "// values == [2, 3]",
                        ],
                    },
                ],
            ),
            array_overloaded_method(
                "splice",
                "Replace a range with new elements and return the removed elements.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![
                                TypeRef::Int,
                                TypeRef::Int,
                                TypeRef::Array(Box::new(TypeRef::Any)),
                            ],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                        },
                        summary: "Replace a fixed number of elements starting at an array index and return the removed elements.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Array index where replacement begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of elements to replace.",
                            },
                            BuiltinParamDoc {
                                name: "replacement",
                                description: "Array of new elements inserted in place of the removed range.",
                            },
                        ],
                        examples: &[
                            "let values = [1, 2, 3, 4];",
                            "let removed = values.splice(1, 2, [20, 30]);",
                            "// removed == [2, 3]",
                            "// values == [1, 20, 30, 4]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range, TypeRef::Array(Box::new(TypeRef::Any))],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                        },
                        summary: "Replace an exclusive range of elements and return the removed elements.",
                        params: &[
                            BuiltinParamDoc {
                                name: "range",
                                description: "Exclusive range of element indexes to replace.",
                            },
                            BuiltinParamDoc {
                                name: "replacement",
                                description: "Array of new elements inserted in place of the removed range.",
                            },
                        ],
                        examples: &[
                            "let values = [1, 2, 3, 4];",
                            "let removed = values.splice(1..3, [20, 30]);",
                            "// removed == [2, 3]",
                            "// values == [1, 20, 30, 4]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![
                                TypeRef::RangeInclusive,
                                TypeRef::Array(Box::new(TypeRef::Any)),
                            ],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                        },
                        summary: "Replace an inclusive range of elements and return the removed elements.",
                        params: &[
                            BuiltinParamDoc {
                                name: "range",
                                description: "Inclusive range of element indexes to replace.",
                            },
                            BuiltinParamDoc {
                                name: "replacement",
                                description: "Array of new elements inserted in place of the removed range.",
                            },
                        ],
                        examples: &[
                            "let values = [1, 2, 3, 4];",
                            "let removed = values.splice(1..=2, [20, 30]);",
                            "// removed == [2, 3]",
                            "// values == [1, 20, 30, 4]",
                        ],
                    },
                ],
            ),
            array_overloaded_method(
                "dedup",
                "Remove consecutive duplicate elements in place.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: Vec::new(),
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Remove consecutive equal elements in place using Rhai equality semantics.",
                        params: &[],
                        examples: &[
                            "let values = [1, 1, 2, 2, 3];",
                            "values.dedup();",
                            "// values == [1, 2, 3]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::FnPtr],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Remove consecutive duplicates in place using a custom comparison callback.",
                        params: &[BuiltinParamDoc {
                            name: "same",
                            description: "Function pointer that returns `true` when two neighboring elements should be treated as duplicates.",
                        }],
                        examples: &[
                            "fn same_parity(left, right) { left % 2 == right % 2 }",
                            "let values = [1, 3, 2, 4, 5];",
                            "values.dedup(Fn(\"same_parity\"));",
                            "// values == [1, 2, 5]",
                        ],
                    },
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
            array_overloaded_method(
                "reduce",
                "Reduce the array into a single value using a callback.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::FnPtr],
                            ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                        },
                        summary: "Fold the array from left to right, using the first element as the initial accumulator.",
                        params: &[BuiltinParamDoc {
                            name: "reducer",
                            description: "Function pointer called with the current accumulator and the next element.",
                        }],
                        examples: &[
                            "fn add(total, value) { total + value }",
                            "let values = [1, 2, 3];",
                            "let sum = values.reduce(Fn(\"add\"));",
                            "// sum == 6",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::FnPtr, TypeRef::Any],
                            ret: Box::new(TypeRef::Any),
                        },
                        summary: "Fold the array from left to right, starting from an explicit initial accumulator value.",
                        params: &[
                            BuiltinParamDoc {
                                name: "reducer",
                                description: "Function pointer called with the current accumulator and the next element.",
                            },
                            BuiltinParamDoc {
                                name: "initial",
                                description: "Initial accumulator value used before the first element is processed.",
                            },
                        ],
                        examples: &[
                            "fn add(total, value) { total + value }",
                            "let values = [1, 2, 3];",
                            "let sum = values.reduce(Fn(\"add\"), 0);",
                            "// sum == 6",
                        ],
                    },
                ],
            ),
            array_overloaded_method(
                "reduce_rev",
                "Reduce the array from the end toward the beginning using a callback.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::FnPtr],
                            ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                        },
                        summary: "Fold the array from right to left, using the last element as the initial accumulator.",
                        params: &[BuiltinParamDoc {
                            name: "reducer",
                            description: "Function pointer called with the current accumulator and the next element from the end.",
                        }],
                        examples: &[
                            "fn append(acc, value) { acc + value }",
                            "let values = [\"a\", \"b\", \"c\"];",
                            "let text = values.reduce_rev(Fn(\"append\"));",
                            "// text == \"cba\"",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::FnPtr, TypeRef::Any],
                            ret: Box::new(TypeRef::Any),
                        },
                        summary: "Fold the array from right to left, starting from an explicit initial accumulator value.",
                        params: &[
                            BuiltinParamDoc {
                                name: "reducer",
                                description: "Function pointer called with the current accumulator and the next element from the end.",
                            },
                            BuiltinParamDoc {
                                name: "initial",
                                description: "Initial accumulator value used before the last element is processed.",
                            },
                        ],
                        examples: &[
                            "fn append(acc, value) { acc + value }",
                            "let values = [\"a\", \"b\", \"c\"];",
                            "let text = values.reduce_rev(Fn(\"append\"), \"\");",
                            "// text == \"cba\"",
                        ],
                    },
                ],
            ),
            array_overloaded_method(
                "find",
                "Return the first element that satisfies a predicate callback.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::FnPtr],
                            ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                        },
                        summary: "Return the first element accepted by a predicate callback, or `()` when none match.",
                        params: &[BuiltinParamDoc {
                            name: "predicate",
                            description: "Function pointer called for each element until it returns `true`.",
                        }],
                        examples: &[
                            "fn is_even(x) { x % 2 == 0 }",
                            "let value = [1, 3, 4, 7].find(Fn(\"is_even\"));",
                            "// value == 4",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::FnPtr, TypeRef::Int],
                            ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                        },
                        summary: "Return the first element accepted by a predicate callback, starting from a given array offset.",
                        params: &[
                            BuiltinParamDoc {
                                name: "predicate",
                                description: "Function pointer called for each element until it returns `true`.",
                            },
                            BuiltinParamDoc {
                                name: "start",
                                description: "Array index where the search begins.",
                            },
                        ],
                        examples: &[
                            "fn is_even(x) { x % 2 == 0 }",
                            "let value = [2, 4, 5, 6].find(Fn(\"is_even\"), 2);",
                            "// value == 6",
                        ],
                    },
                ],
            ),
            array_overloaded_method(
                "find_map",
                "Return the first non-unit value produced by the callback.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::FnPtr],
                            ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                        },
                        summary: "Return the first non-unit value produced by a callback, or `()` when the callback never produces a value.",
                        params: &[BuiltinParamDoc {
                            name: "mapper",
                            description: "Function pointer called for each element until it returns something other than `()`.",
                        }],
                        examples: &[
                            "fn parse_even(x) { if x % 2 == 0 { x * 10 } else { () } }",
                            "let value = [1, 3, 4, 7].find_map(Fn(\"parse_even\"));",
                            "// value == 40",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::FnPtr, TypeRef::Int],
                            ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                        },
                        summary: "Return the first non-unit value produced by a callback, starting from a given array offset.",
                        params: &[
                            BuiltinParamDoc {
                                name: "mapper",
                                description: "Function pointer called for each element until it returns something other than `()`.",
                            },
                            BuiltinParamDoc {
                                name: "start",
                                description: "Array index where the search begins.",
                            },
                        ],
                        examples: &[
                            "fn parse_even(x) { if x % 2 == 0 { x * 10 } else { () } }",
                            "let value = [2, 4, 6].find_map(Fn(\"parse_even\"), 1);",
                            "// value == 40",
                        ],
                    },
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
