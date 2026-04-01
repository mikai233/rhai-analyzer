use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::docs::builtin_type_docs;
use crate::builtin::signatures::helpers::builtin_documented_method;
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
            string_method(
                "pad",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Int, TypeRef::Char],
                        ret: Box::new(TypeRef::Unit),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int, TypeRef::String],
                        ret: Box::new(TypeRef::Unit),
                    },
                ],
                "Pad the string to the target length.",
                &[
                    "let text = \"42\";",
                    "text.pad(5, '0');",
                    "// text == \"42000\"",
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
            string_method(
                "remove",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Char],
                        ret: Box::new(TypeRef::Unit),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String],
                        ret: Box::new(TypeRef::Unit),
                    },
                ],
                "Remove a character or substring from the string.",
                &[
                    "let text = \"banana\";",
                    "text.remove(\"na\");",
                    "// text == \"ba\"",
                ],
            ),
            string_method(
                "pop",
                vec![
                    FunctionTypeRef {
                        params: Vec::new(),
                        ret: Box::new(TypeRef::Union(vec![TypeRef::Char, TypeRef::Unit])),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int],
                        ret: Box::new(TypeRef::String),
                    },
                ],
                "Remove characters from the end of the string.",
                &[
                    "let text = \"hello\";",
                    "let last = text.pop();",
                    "// last == 'o'",
                    "// text == \"hell\"",
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
            string_method(
                "contains",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Char],
                        ret: Box::new(TypeRef::Bool),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String],
                        ret: Box::new(TypeRef::Bool),
                    },
                ],
                "Check whether the string contains a character or substring.",
                &[
                    "let found = \"hello\".contains(\"ell\");",
                    "// found == true",
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
            string_method(
                "index_of",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Char],
                        ret: Box::new(TypeRef::Int),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Char, TypeRef::Int],
                        ret: Box::new(TypeRef::Int),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String],
                        ret: Box::new(TypeRef::Int),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String, TypeRef::Int],
                        ret: Box::new(TypeRef::Int),
                    },
                ],
                "Find the position of a character or substring.",
                &["let pos = \"hello\".index_of(\"ll\");", "// pos == 2"],
            ),
            string_method(
                "sub_string",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Int],
                        ret: Box::new(TypeRef::String),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int, TypeRef::Int],
                        ret: Box::new(TypeRef::String),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Range],
                        ret: Box::new(TypeRef::String),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::RangeInclusive],
                        ret: Box::new(TypeRef::String),
                    },
                ],
                "Extract a substring from the string.",
                &[
                    "let part = \"hello\".sub_string(1, 3);",
                    "// part == \"ell\"",
                ],
            ),
            string_method(
                "split",
                vec![
                    FunctionTypeRef {
                        params: Vec::new(),
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Char],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Char, TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String, TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                ],
                "Split the string into segments.",
                &[
                    "let parts = \"a,b,c\".split(\",\");",
                    "// parts == [\"a\", \"b\", \"c\"]",
                ],
            ),
            string_method(
                "split_rev",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Char],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Char, TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String, TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                ],
                "Split the string in reverse order.",
                &[
                    "let parts = \"a,b,c\".split_rev(\",\", 2);",
                    "// parts == [\"c\", \"a,b\"]",
                ],
            ),
            string_method(
                "crop",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Int],
                        ret: Box::new(TypeRef::Unit),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int, TypeRef::Int],
                        ret: Box::new(TypeRef::Unit),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Range],
                        ret: Box::new(TypeRef::Unit),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::RangeInclusive],
                        ret: Box::new(TypeRef::Unit),
                    },
                ],
                "Retain only a portion of the string.",
                &[
                    "let text = \"hello\";",
                    "text.crop(1, 3);",
                    "// text == \"ell\"",
                ],
            ),
            string_method(
                "replace",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Char, TypeRef::Char],
                        ret: Box::new(TypeRef::Unit),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String, TypeRef::String],
                        ret: Box::new(TypeRef::Unit),
                    },
                ],
                "Replace a character or substring in place.",
                &[
                    "let text = \"hello\";",
                    "text.replace(\"ll\", \"yy\");",
                    "// text == \"heyyo\"",
                ],
            ),
            string_method(
                "chars",
                vec![
                    FunctionTypeRef {
                        params: Vec::new(),
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Char))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Char))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int, TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Char))),
                    },
                ],
                "Collect characters from the string into an array.",
                &[
                    "let chars = \"hello\".chars();",
                    "// chars == ['h', 'e', 'l', 'l', 'o']",
                ],
            ),
        ],
    }
}
