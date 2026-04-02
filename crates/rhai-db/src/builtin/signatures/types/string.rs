use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::docs::{
    BuiltinCallableOverloadDoc, BuiltinParamDoc, builtin_type_docs,
};
use crate::builtin::signatures::helpers::{
    builtin_documented_method, builtin_documented_overloaded_method,
};
use crate::types::{HostFunction, HostType};

const STRING_REFERENCE_URL: &str = "https://rhai.rs/book/ref/strings-chars.html";

fn string_method(
    name: &str,
    signatures: Vec<FunctionTypeRef>,
    summary: &str,
    examples: &[&str],
) -> HostFunction {
    builtin_documented_method(
        "string",
        name,
        signatures,
        summary,
        examples,
        STRING_REFERENCE_URL,
    )
}

fn string_overloaded_method(
    name: &str,
    summary: &str,
    overloads: Vec<BuiltinCallableOverloadDoc<'_>>,
) -> HostFunction {
    builtin_documented_overloaded_method("string", name, summary, overloads, STRING_REFERENCE_URL)
}

pub(crate) fn builtin_string_type() -> HostType {
    HostType {
        name: "string".to_owned(),
        generic_params: Vec::new(),
        docs: Some(builtin_type_docs(
            "string",
            "Builtin Rhai string type for UTF-8 text and character-oriented string processing.",
            &[
                "let text = \"hello\";",
                "let upper = text.to_upper();",
                "// upper == \"HELLO\"",
            ],
            STRING_REFERENCE_URL,
        )),
        methods: vec![
            string_method(
                "len",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                "Return the number of characters in the string.",
                &["let count = \"hello\".len();", "// count == 5"],
            ),
            string_method(
                "bytes",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                "Return the number of UTF-8 bytes used by the string.",
                &["let bytes = \"hello\".bytes();", "// bytes == 5"],
            ),
            string_method(
                "is_empty",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if the string is empty.",
                &["let empty = \"\".is_empty();", "// empty == true"],
            ),
            string_method(
                "to_blob",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Blob),
                }],
                "Convert the string into a UTF-8 encoded BLOB.",
                &[
                    "let bytes = \"hello\".to_blob();",
                    "// bytes == [104, 101, 108, 108, 111]",
                ],
            ),
            string_method(
                "to_chars",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::Char))),
                }],
                "Split the string into an array of individual characters.",
                &[
                    "let chars = \"hello\".to_chars();",
                    "// chars == ['h', 'e', 'l', 'l', 'o']",
                ],
            ),
            string_method(
                "get",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Union(vec![TypeRef::Char, TypeRef::Unit])),
                }],
                "Get the character at the specified position.",
                &["let first = \"hello\".get(0);", "// first == 'h'"],
            ),
            string_method(
                "set",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int, TypeRef::Char],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Set the character at the specified position.",
                &[
                    "let text = \"hello\";",
                    "text.set(0, 'H');",
                    "// text == \"Hello\"",
                ],
            ),
            string_overloaded_method(
                "pad",
                "Pad the string to the target length.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Char],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Extend the string to the requested character length by repeating a single padding character.",
                        params: &[
                            BuiltinParamDoc {
                                name: "len",
                                description: "Target character length after padding.",
                            },
                            BuiltinParamDoc {
                                name: "padding",
                                description: "Single character appended until the string reaches the requested length.",
                            },
                        ],
                        examples: &[
                            "let text = \"42\";",
                            "text.pad(5, '0');",
                            "// text == \"42000\"",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::String],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Extend the string to the requested character length by repeating a padding string.",
                        params: &[
                            BuiltinParamDoc {
                                name: "len",
                                description: "Target character length after padding.",
                            },
                            BuiltinParamDoc {
                                name: "padding",
                                description: "String segment appended repeatedly until the target length is reached.",
                            },
                        ],
                        examples: &[
                            "let text = \"ha\";",
                            "text.pad(6, \"!\");",
                            "// text == \"ha!!!!\"",
                        ],
                    },
                ],
            ),
            string_method(
                "append",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Any],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Append a value to the end of the string.",
                &[
                    "let text = \"item: \";",
                    "text.append(42);",
                    "// text == \"item: 42\"",
                ],
            ),
            string_overloaded_method(
                "remove",
                "Remove a character or substring from the string.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Char],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Remove every occurrence of a single character from the string in place.",
                        params: &[BuiltinParamDoc {
                            name: "needle",
                            description: "Character to remove wherever it appears.",
                        }],
                        examples: &[
                            "let text = \"banana\";",
                            "text.remove('a');",
                            "// text == \"bnn\"",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::String],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Remove every occurrence of a substring from the string in place.",
                        params: &[BuiltinParamDoc {
                            name: "needle",
                            description: "Substring to remove wherever it appears.",
                        }],
                        examples: &[
                            "let text = \"banana\";",
                            "text.remove(\"na\");",
                            "// text == \"ba\"",
                        ],
                    },
                ],
            ),
            string_overloaded_method(
                "pop",
                "Remove characters from the end of the string.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: Vec::new(),
                            ret: Box::new(TypeRef::Union(vec![TypeRef::Char, TypeRef::Unit])),
                        },
                        summary: "Remove the last character and return it, or `()` when the string is empty.",
                        params: &[],
                        examples: &[
                            "let text = \"hello\";",
                            "let last = text.pop();",
                            "// last == 'o'",
                            "// text == \"hell\"",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int],
                            ret: Box::new(TypeRef::String),
                        },
                        summary: "Remove a suffix with the requested number of characters and return the removed substring.",
                        params: &[BuiltinParamDoc {
                            name: "count",
                            description: "How many trailing characters to remove and return.",
                        }],
                        examples: &[
                            "let text = \"hello\";",
                            "let suffix = text.pop(2);",
                            "// suffix == \"lo\"",
                            "// text == \"hel\"",
                        ],
                    },
                ],
            ),
            string_method(
                "clear",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                "Clear the string in place.",
                &["let text = \"hello\";", "text.clear();", "// text == \"\""],
            ),
            string_method(
                "truncate",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Truncate the string to the specified length.",
                &[
                    "let text = \"hello\";",
                    "text.truncate(3);",
                    "// text == \"hel\"",
                ],
            ),
            string_method(
                "to_upper",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::String),
                }],
                "Return an upper-case copy of the string.",
                &["let upper = \"hello\".to_upper();", "// upper == \"HELLO\""],
            ),
            string_method(
                "to_lower",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::String),
                }],
                "Return a lower-case copy of the string.",
                &["let lower = \"HELLO\".to_lower();", "// lower == \"hello\""],
            ),
            string_method(
                "make_upper",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                "Convert the string to upper-case in place.",
                &[
                    "let text = \"hello\";",
                    "text.make_upper();",
                    "// text == \"HELLO\"",
                ],
            ),
            string_method(
                "make_lower",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                "Convert the string to lower-case in place.",
                &[
                    "let text = \"HELLO\";",
                    "text.make_lower();",
                    "// text == \"hello\"",
                ],
            ),
            string_method(
                "trim",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                "Trim leading and trailing whitespace in place.",
                &[
                    "let text = \"  hello  \";",
                    "text.trim();",
                    "// text == \"hello\"",
                ],
            ),
            string_overloaded_method(
                "contains",
                "Check whether the string contains a character or substring.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Char],
                            ret: Box::new(TypeRef::Bool),
                        },
                        summary: "Return `true` when the string contains the requested character.",
                        params: &[BuiltinParamDoc {
                            name: "needle",
                            description: "Character to search for.",
                        }],
                        examples: &["let found = \"hello\".contains('e');", "// found == true"],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::String],
                            ret: Box::new(TypeRef::Bool),
                        },
                        summary: "Return `true` when the string contains the requested substring.",
                        params: &[BuiltinParamDoc {
                            name: "needle",
                            description: "Substring to search for.",
                        }],
                        examples: &[
                            "let found = \"hello\".contains(\"ell\");",
                            "// found == true",
                        ],
                    },
                ],
            ),
            string_method(
                "starts_with",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::Bool),
                }],
                "Check whether the string starts with the given prefix.",
                &["let ok = \"prefix\".starts_with(\"pre\");", "// ok == true"],
            ),
            string_method(
                "ends_with",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::Bool),
                }],
                "Check whether the string ends with the given suffix.",
                &[
                    "let ok = \"main.rhai\".ends_with(\".rhai\");",
                    "// ok == true",
                ],
            ),
            string_overloaded_method(
                "index_of",
                "Find the position of a character or substring.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Char],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Return the position of the first matching character, or `-1` when it is absent.",
                        params: &[BuiltinParamDoc {
                            name: "needle",
                            description: "Character to search for.",
                        }],
                        examples: &["let pos = \"hello\".index_of('l');", "// pos == 2"],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Char, TypeRef::Int],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Return the position of the first matching character, starting from a given offset.",
                        params: &[
                            BuiltinParamDoc {
                                name: "needle",
                                description: "Character to search for.",
                            },
                            BuiltinParamDoc {
                                name: "start",
                                description: "Character offset where the search begins.",
                            },
                        ],
                        examples: &["let pos = \"hello\".index_of('l', 3);", "// pos == 3"],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::String],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Return the position of the first matching substring, or `-1` when it is absent.",
                        params: &[BuiltinParamDoc {
                            name: "needle",
                            description: "Substring to search for.",
                        }],
                        examples: &["let pos = \"hello\".index_of(\"ll\");", "// pos == 2"],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::String, TypeRef::Int],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Return the position of the first matching substring, starting from a given offset.",
                        params: &[
                            BuiltinParamDoc {
                                name: "needle",
                                description: "Substring to search for.",
                            },
                            BuiltinParamDoc {
                                name: "start",
                                description: "Character offset where the search begins.",
                            },
                        ],
                        examples: &["let pos = \"hello\".index_of(\"l\", 3);", "// pos == 3"],
                    },
                ],
            ),
            string_overloaded_method(
                "sub_string",
                "Extract a substring from the string.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int],
                            ret: Box::new(TypeRef::String),
                        },
                        summary: "Return the suffix beginning at the requested character offset.",
                        params: &[BuiltinParamDoc {
                            name: "start",
                            description: "Character offset where the returned substring begins.",
                        }],
                        examples: &["let part = \"hello\".sub_string(2);", "// part == \"llo\""],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int],
                            ret: Box::new(TypeRef::String),
                        },
                        summary: "Return a substring with a starting offset and explicit character length.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Character offset where the returned substring begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of characters to copy.",
                            },
                        ],
                        examples: &[
                            "let part = \"hello\".sub_string(1, 3);",
                            "// part == \"ell\"",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range],
                            ret: Box::new(TypeRef::String),
                        },
                        summary: "Return a substring selected by an exclusive range of character offsets.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Exclusive character range to copy.",
                        }],
                        examples: &[
                            "let part = \"hello\".sub_string(1..4);",
                            "// part == \"ell\"",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive],
                            ret: Box::new(TypeRef::String),
                        },
                        summary: "Return a substring selected by an inclusive range of character offsets.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Inclusive character range to copy.",
                        }],
                        examples: &[
                            "let part = \"hello\".sub_string(1..=3);",
                            "// part == \"ell\"",
                        ],
                    },
                ],
            ),
            string_overloaded_method(
                "split",
                "Split the string into segments.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: Vec::new(),
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                        },
                        summary: "Split the string on whitespace boundaries.",
                        params: &[],
                        examples: &[
                            "let parts = \"a b  c\".split();",
                            "// parts == [\"a\", \"b\", \"c\"]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                        },
                        summary: "Split the string on whitespace boundaries, limiting the number of returned segments.",
                        params: &[BuiltinParamDoc {
                            name: "limit",
                            description: "Maximum number of segments to return.",
                        }],
                        examples: &[
                            "let parts = \"a b c d\".split(2);",
                            "// parts == [\"a\", \"b c d\"]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Char],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                        },
                        summary: "Split the string using a single-character delimiter.",
                        params: &[BuiltinParamDoc {
                            name: "delimiter",
                            description: "Character used to split the string.",
                        }],
                        examples: &[
                            "let parts = \"a,b,c\".split(',');",
                            "// parts == [\"a\", \"b\", \"c\"]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Char, TypeRef::Int],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                        },
                        summary: "Split the string using a single-character delimiter, limiting the number of returned segments.",
                        params: &[
                            BuiltinParamDoc {
                                name: "delimiter",
                                description: "Character used to split the string.",
                            },
                            BuiltinParamDoc {
                                name: "limit",
                                description: "Maximum number of segments to return.",
                            },
                        ],
                        examples: &[
                            "let parts = \"a,b,c\".split(',', 2);",
                            "// parts == [\"a\", \"b,c\"]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::String],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                        },
                        summary: "Split the string using a substring delimiter.",
                        params: &[BuiltinParamDoc {
                            name: "delimiter",
                            description: "Substring used to split the string.",
                        }],
                        examples: &[
                            "let parts = \"a--b--c\".split(\"--\");",
                            "// parts == [\"a\", \"b\", \"c\"]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::String, TypeRef::Int],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                        },
                        summary: "Split the string using a substring delimiter, limiting the number of returned segments.",
                        params: &[
                            BuiltinParamDoc {
                                name: "delimiter",
                                description: "Substring used to split the string.",
                            },
                            BuiltinParamDoc {
                                name: "limit",
                                description: "Maximum number of segments to return.",
                            },
                        ],
                        examples: &[
                            "let parts = \"a--b--c\".split(\"--\", 2);",
                            "// parts == [\"a\", \"b--c\"]",
                        ],
                    },
                ],
            ),
            string_overloaded_method(
                "split_rev",
                "Split the string in reverse order.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Char],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                        },
                        summary: "Split the string from the end using a single-character delimiter.",
                        params: &[BuiltinParamDoc {
                            name: "delimiter",
                            description: "Character used to split the string from right to left.",
                        }],
                        examples: &[
                            "let parts = \"a,b,c\".split_rev(',');",
                            "// parts == [\"c\", \"b\", \"a\"]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Char, TypeRef::Int],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                        },
                        summary: "Split the string from the end using a single-character delimiter, limiting the number of returned segments.",
                        params: &[
                            BuiltinParamDoc {
                                name: "delimiter",
                                description: "Character used to split the string from right to left.",
                            },
                            BuiltinParamDoc {
                                name: "limit",
                                description: "Maximum number of segments to return.",
                            },
                        ],
                        examples: &[
                            "let parts = \"a,b,c\".split_rev(',', 2);",
                            "// parts == [\"c\", \"a,b\"]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::String],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                        },
                        summary: "Split the string from the end using a substring delimiter.",
                        params: &[BuiltinParamDoc {
                            name: "delimiter",
                            description: "Substring used to split the string from right to left.",
                        }],
                        examples: &[
                            "let parts = \"a--b--c\".split_rev(\"--\");",
                            "// parts == [\"c\", \"b\", \"a\"]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::String, TypeRef::Int],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                        },
                        summary: "Split the string from the end using a substring delimiter, limiting the number of returned segments.",
                        params: &[
                            BuiltinParamDoc {
                                name: "delimiter",
                                description: "Substring used to split the string from right to left.",
                            },
                            BuiltinParamDoc {
                                name: "limit",
                                description: "Maximum number of segments to return.",
                            },
                        ],
                        examples: &[
                            "let parts = \"a--b--c\".split_rev(\"--\", 2);",
                            "// parts == [\"c\", \"a--b\"]",
                        ],
                    },
                ],
            ),
            string_overloaded_method(
                "crop",
                "Retain only a portion of the string.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Keep only the suffix beginning at the requested character offset.",
                        params: &[BuiltinParamDoc {
                            name: "start",
                            description: "Character offset where the retained suffix begins.",
                        }],
                        examples: &[
                            "let text = \"hello\";",
                            "text.crop(2);",
                            "// text == \"llo\"",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Keep a substring in place using a starting offset and explicit character length.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Character offset where the retained substring begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of characters to keep.",
                            },
                        ],
                        examples: &[
                            "let text = \"hello\";",
                            "text.crop(1, 3);",
                            "// text == \"ell\"",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Keep only the characters selected by an exclusive range of offsets.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Exclusive character range to retain.",
                        }],
                        examples: &[
                            "let text = \"hello\";",
                            "text.crop(1..4);",
                            "// text == \"ell\"",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Keep only the characters selected by an inclusive range of offsets.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Inclusive character range to retain.",
                        }],
                        examples: &[
                            "let text = \"hello\";",
                            "text.crop(1..=3);",
                            "// text == \"ell\"",
                        ],
                    },
                ],
            ),
            string_overloaded_method(
                "replace",
                "Replace a character or substring in place.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Char, TypeRef::Char],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Replace every occurrence of one character with another in place.",
                        params: &[
                            BuiltinParamDoc {
                                name: "needle",
                                description: "Character to replace.",
                            },
                            BuiltinParamDoc {
                                name: "replacement",
                                description: "Character written in place of each match.",
                            },
                        ],
                        examples: &[
                            "let text = \"hello\";",
                            "text.replace('l', 'y');",
                            "// text == \"heyyo\"",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::String, TypeRef::String],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Replace every occurrence of one substring with another in place.",
                        params: &[
                            BuiltinParamDoc {
                                name: "needle",
                                description: "Substring to replace.",
                            },
                            BuiltinParamDoc {
                                name: "replacement",
                                description: "Substring written in place of each match.",
                            },
                        ],
                        examples: &[
                            "let text = \"hello\";",
                            "text.replace(\"ll\", \"yy\");",
                            "// text == \"heyyo\"",
                        ],
                    },
                ],
            ),
            string_overloaded_method(
                "chars",
                "Collect characters from the string into an array.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: Vec::new(),
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Char))),
                        },
                        summary: "Collect every character in the string into an array.",
                        params: &[],
                        examples: &[
                            "let chars = \"hello\".chars();",
                            "// chars == ['h', 'e', 'l', 'l', 'o']",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Char))),
                        },
                        summary: "Collect characters starting at a given character offset.",
                        params: &[BuiltinParamDoc {
                            name: "start",
                            description: "Character offset where collection begins.",
                        }],
                        examples: &[
                            "let chars = \"hello\".chars(2);",
                            "// chars == ['l', 'l', 'o']",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int],
                            ret: Box::new(TypeRef::Array(Box::new(TypeRef::Char))),
                        },
                        summary: "Collect a fixed number of characters starting at a given offset.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Character offset where collection begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Maximum number of characters to collect.",
                            },
                        ],
                        examples: &[
                            "let chars = \"hello\".chars(1, 3);",
                            "// chars == ['e', 'l', 'l']",
                        ],
                    },
                ],
            ),
        ],
    }
}
