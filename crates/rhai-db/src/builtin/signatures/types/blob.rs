use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::docs::{
    BuiltinCallableOverloadDoc, BuiltinParamDoc, builtin_type_docs,
};
use crate::builtin::signatures::helpers::{
    builtin_documented_method, builtin_documented_overloaded_method,
};
use crate::types::{HostFunction, HostType};

const BLOB_REFERENCE_URL: &str = "https://rhai.rs/book/language/blobs.html";

fn blob_method(
    name: &str,
    signatures: Vec<FunctionTypeRef>,
    summary: &str,
    examples: &[&str],
) -> HostFunction {
    builtin_documented_method(
        "blob",
        name,
        signatures,
        summary,
        examples,
        BLOB_REFERENCE_URL,
    )
}

fn blob_overloaded_method(
    name: &str,
    summary: &str,
    overloads: Vec<BuiltinCallableOverloadDoc<'_>>,
) -> HostFunction {
    builtin_documented_overloaded_method("blob", name, summary, overloads, BLOB_REFERENCE_URL)
}

pub(crate) fn builtin_blob_type() -> HostType {
    HostType {
        name: "blob".to_owned(),
        generic_params: Vec::new(),
        docs: Some(builtin_type_docs(
            "blob",
            "Builtin Rhai BLOB type for byte-oriented buffers and binary data.",
            &[
                "let buf = blob(2, 0);",
                "buf.push(255);",
                "// buf == [0, 0, 255]",
            ],
            BLOB_REFERENCE_URL,
        )),
        methods: vec![
            blob_method(
                "get",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Int),
                }],
                "Get a copy of the byte at the specified position.",
                &[
                    "let buf = blob(3, 7);",
                    "let byte = buf.get(1);",
                    "// byte == 7",
                ],
            ),
            blob_method(
                "set",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int, TypeRef::Int],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Set the byte at the specified position.",
                &[
                    "let buf = blob(3, 0);",
                    "buf.set(1, 255);",
                    "// buf == [0, 255, 0]",
                ],
            ),
            blob_method(
                "push",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Append a byte to the end of the BLOB.",
                &[
                    "let buf = blob(2, 0);",
                    "buf.push(255);",
                    "// buf == [0, 0, 255]",
                ],
            ),
            blob_overloaded_method(
                "append",
                "Append another BLOB, string, or character to the end of this BLOB.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Blob],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Append the bytes from another BLOB to the end of this BLOB.",
                        params: &[BuiltinParamDoc {
                            name: "other",
                            description: "BLOB whose bytes are appended in order.",
                        }],
                        examples: &[
                            "let buf = blob(2, 0);",
                            "buf.append(blob(2, 1));",
                            "// buf == [0, 0, 1, 1]",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::String],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Append the UTF-8 bytes of a string to the end of this BLOB.",
                        params: &[BuiltinParamDoc {
                            name: "text",
                            description: "String encoded as UTF-8 and appended byte by byte.",
                        }],
                        examples: &[
                            "let buf = blob(2, 0);",
                            "buf.append(\"A\");",
                            "// buf now ends with the UTF-8 bytes for \"A\"",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Char],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Append the UTF-8 bytes of a single character to the end of this BLOB.",
                        params: &[BuiltinParamDoc {
                            name: "ch",
                            description: "Character encoded as UTF-8 and appended byte by byte.",
                        }],
                        examples: &["let buf = blob();", "buf.append('A');", "// buf == [65]"],
                    },
                ],
            ),
            blob_method(
                "insert",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int, TypeRef::Int],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Insert a byte at the specified position.",
                &[
                    "let buf = blob(2, 0);",
                    "buf.insert(1, 255);",
                    "// buf == [0, 255, 0]",
                ],
            ),
            blob_method(
                "pop",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                "Remove the last byte and return it.",
                &[
                    "let buf = blob(2, 1);",
                    "let last = buf.pop();",
                    "// last == 1",
                    "// buf == [1]",
                ],
            ),
            blob_method(
                "shift",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                "Remove the first byte and return it.",
                &[
                    "let buf = blob(2, 1);",
                    "let first = buf.shift();",
                    "// first == 1",
                    "// buf == [1]",
                ],
            ),
            blob_method(
                "remove",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Int),
                }],
                "Remove the byte at the specified position.",
                &[
                    "let buf = blob(3, 7);",
                    "let removed = buf.remove(1);",
                    "// removed == 7",
                    "// buf == [7, 7]",
                ],
            ),
            blob_method(
                "reverse",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                "Reverse the BLOB byte by byte.",
                &[
                    "let buf = blob();",
                    "buf.push(1);",
                    "buf.push(2);",
                    "buf.push(3);",
                    "buf.reverse();",
                    "// buf == [3, 2, 1]",
                ],
            ),
            blob_method(
                "as_string",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::String),
                }],
                "Interpret the BLOB as UTF-8 and return it as a string.",
                &[
                    "let text = \"hi\".to_blob().as_string();",
                    "// text == \"hi\"",
                ],
            ),
            blob_method(
                "len",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                "Return the number of bytes in the BLOB.",
                &["let count = blob(4, 0).len();", "// count == 4"],
            ),
            blob_method(
                "is_empty",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if the BLOB is empty.",
                &["let empty = blob().is_empty();", "// empty == true"],
            ),
            blob_method(
                "clear",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                "Clear the BLOB in place.",
                &["let buf = blob(3, 7);", "buf.clear();", "// buf == []"],
            ),
            blob_method(
                "truncate",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Truncate the BLOB to the specified length.",
                &[
                    "let buf = blob(4, 9);",
                    "buf.push(1);",
                    "buf.push(2);",
                    "buf.truncate(3);",
                    "// buf == [9, 9, 9]",
                ],
            ),
            blob_method(
                "pad",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int, TypeRef::Int],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Pad the BLOB with a byte value to at least the target length.",
                &[
                    "let buf = blob(2, 1);",
                    "buf.pad(4, 9);",
                    "// buf == [1, 1, 9, 9]",
                ],
            ),
            blob_method(
                "split",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::Blob))),
                }],
                "Split the BLOB into two BLOBs at the specified position.",
                &[
                    "let parts = blob(4, 1).split(2);",
                    "// parts == [blob(2, 1), blob(2, 1)] conceptually",
                ],
            ),
            blob_method(
                "contains",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Bool),
                }],
                "Check whether the BLOB contains a byte value.",
                &[
                    "let found = blob(2, 255).contains(255);",
                    "// found == true",
                ],
            ),
            blob_method(
                "to_array",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::Int))),
                }],
                "Convert the BLOB into an array of integers.",
                &[
                    "let bytes = blob(3, 1).to_array();",
                    "// bytes == [1, 1, 1]",
                ],
            ),
            blob_overloaded_method(
                "extract",
                "Extract a portion of the BLOB into a new BLOB.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int],
                            ret: Box::new(TypeRef::Blob),
                        },
                        summary: "Copy a portion of the BLOB using a starting byte offset and an explicit byte length.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Byte offset where extraction begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of bytes to copy into the returned BLOB.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(4, 7);",
                            "let part = buf.extract(1, 2);",
                            "// part contains the middle bytes",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range],
                            ret: Box::new(TypeRef::Blob),
                        },
                        summary: "Copy a portion of the BLOB selected by an exclusive byte range.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Exclusive byte range to copy.",
                        }],
                        examples: &[
                            "let buf = blob(4, 7);",
                            "let part = buf.extract(1..3);",
                            "// part contains the middle bytes",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive],
                            ret: Box::new(TypeRef::Blob),
                        },
                        summary: "Copy a portion of the BLOB selected by an inclusive byte range.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Inclusive byte range to copy.",
                        }],
                        examples: &[
                            "let buf = blob(4, 7);",
                            "let part = buf.extract(1..=2);",
                            "// part contains the middle bytes",
                        ],
                    },
                ],
            ),
            blob_method(
                "chop",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Blob),
                }],
                "Cut off the head of the BLOB, leaving the tail at the target length.",
                &[
                    "let buf = blob(4, 7);",
                    "let removed = buf.chop(2);",
                    "// removed contains the head bytes",
                    "// buf keeps the last two bytes",
                ],
            ),
            blob_overloaded_method(
                "drain",
                "Remove a portion of the BLOB, returning the removed bytes as a new BLOB.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int],
                            ret: Box::new(TypeRef::Blob),
                        },
                        summary: "Remove a portion of the BLOB using a starting byte offset and explicit byte length.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Byte offset where removal begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of bytes to remove.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(4, 7);",
                            "let removed = buf.drain(1, 2);",
                            "// removed contains the removed bytes",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range],
                            ret: Box::new(TypeRef::Blob),
                        },
                        summary: "Remove a portion of the BLOB selected by an exclusive byte range.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Exclusive byte range to remove.",
                        }],
                        examples: &[
                            "let buf = blob(4, 7);",
                            "let removed = buf.drain(1..3);",
                            "// removed contains the removed bytes",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive],
                            ret: Box::new(TypeRef::Blob),
                        },
                        summary: "Remove a portion of the BLOB selected by an inclusive byte range.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Inclusive byte range to remove.",
                        }],
                        examples: &[
                            "let buf = blob(4, 7);",
                            "let removed = buf.drain(1..=2);",
                            "// removed contains the removed bytes",
                        ],
                    },
                ],
            ),
            blob_overloaded_method(
                "retain",
                "Retain a portion of the BLOB, returning the removed bytes as a new BLOB.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int],
                            ret: Box::new(TypeRef::Blob),
                        },
                        summary: "Keep a fixed number of bytes starting at a byte offset and return the removed bytes as a new BLOB.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Byte offset where the retained window begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of bytes to keep.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(4, 7);",
                            "let removed = buf.retain(1, 2);",
                            "// removed contains the bytes outside the retained range",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range],
                            ret: Box::new(TypeRef::Blob),
                        },
                        summary: "Keep bytes selected by an exclusive byte range and return the removed bytes as a new BLOB.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Exclusive byte range to keep.",
                        }],
                        examples: &[
                            "let buf = blob(4, 7);",
                            "let removed = buf.retain(1..3);",
                            "// removed contains the bytes outside the retained range",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive],
                            ret: Box::new(TypeRef::Blob),
                        },
                        summary: "Keep bytes selected by an inclusive byte range and return the removed bytes as a new BLOB.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Inclusive byte range to keep.",
                        }],
                        examples: &[
                            "let buf = blob(4, 7);",
                            "let removed = buf.retain(1..=2);",
                            "// removed contains the bytes outside the retained range",
                        ],
                    },
                ],
            ),
            blob_overloaded_method(
                "splice",
                "Replace a portion of the BLOB with another BLOB, returning the removed bytes.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int, TypeRef::Blob],
                            ret: Box::new(TypeRef::Blob),
                        },
                        summary: "Replace a fixed number of bytes starting at a byte offset and return the removed bytes.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Byte offset where replacement begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of bytes to replace.",
                            },
                            BuiltinParamDoc {
                                name: "replacement",
                                description: "BLOB inserted in place of the removed bytes.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(4, 7);",
                            "let removed = buf.splice(1, 2, blob(2, 9));",
                            "// removed contains the replaced bytes",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range, TypeRef::Blob],
                            ret: Box::new(TypeRef::Blob),
                        },
                        summary: "Replace bytes selected by an exclusive range and return the removed bytes.",
                        params: &[
                            BuiltinParamDoc {
                                name: "range",
                                description: "Exclusive byte range to replace.",
                            },
                            BuiltinParamDoc {
                                name: "replacement",
                                description: "BLOB inserted in place of the removed bytes.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(4, 7);",
                            "let removed = buf.splice(1..3, blob(2, 9));",
                            "// removed contains the replaced bytes",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive, TypeRef::Blob],
                            ret: Box::new(TypeRef::Blob),
                        },
                        summary: "Replace bytes selected by an inclusive range and return the removed bytes.",
                        params: &[
                            BuiltinParamDoc {
                                name: "range",
                                description: "Inclusive byte range to replace.",
                            },
                            BuiltinParamDoc {
                                name: "replacement",
                                description: "BLOB inserted in place of the removed bytes.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(4, 7);",
                            "let removed = buf.splice(1..=2, blob(2, 9));",
                            "// removed contains the replaced bytes",
                        ],
                    },
                ],
            ),
            blob_overloaded_method(
                "parse_le_int",
                "Parse an integer from the BLOB in little-endian byte order.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Parse an integer from a fixed byte window in little-endian byte order.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Byte offset where parsing begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of bytes to interpret as an integer.",
                            },
                        ],
                        examples: &[
                            "let value = blob(2, 1).parse_le_int(0, 2);",
                            "// value parses the first bytes as a little-endian integer",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Parse an integer from bytes selected by an exclusive range in little-endian byte order.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Exclusive byte range to parse as an integer.",
                        }],
                        examples: &[
                            "let value = blob(2, 1).parse_le_int(0..2);",
                            "// value parses the selected bytes as a little-endian integer",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Parse an integer from bytes selected by an inclusive range in little-endian byte order.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Inclusive byte range to parse as an integer.",
                        }],
                        examples: &[
                            "let value = blob(2, 1).parse_le_int(0..=1);",
                            "// value parses the selected bytes as a little-endian integer",
                        ],
                    },
                ],
            ),
            blob_overloaded_method(
                "parse_be_int",
                "Parse an integer from the BLOB in big-endian byte order.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Parse an integer from a fixed byte window in big-endian byte order.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Byte offset where parsing begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of bytes to interpret as an integer.",
                            },
                        ],
                        examples: &[
                            "let value = blob(2, 1).parse_be_int(0, 2);",
                            "// value parses the first bytes as a big-endian integer",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Parse an integer from bytes selected by an exclusive range in big-endian byte order.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Exclusive byte range to parse as an integer.",
                        }],
                        examples: &[
                            "let value = blob(2, 1).parse_be_int(0..2);",
                            "// value parses the selected bytes as a big-endian integer",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Parse an integer from bytes selected by an inclusive range in big-endian byte order.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Inclusive byte range to parse as an integer.",
                        }],
                        examples: &[
                            "let value = blob(2, 1).parse_be_int(0..=1);",
                            "// value parses the selected bytes as a big-endian integer",
                        ],
                    },
                ],
            ),
            blob_overloaded_method(
                "parse_le_float",
                "Parse a floating-point number from the BLOB in little-endian byte order.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int],
                            ret: Box::new(TypeRef::Float),
                        },
                        summary: "Parse a floating-point number from a fixed byte window in little-endian byte order.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Byte offset where parsing begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of bytes to interpret as a floating-point value.",
                            },
                        ],
                        examples: &[
                            "let value = blob(8, 0).parse_le_float(0, 8);",
                            "// value parses the bytes as a little-endian float",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range],
                            ret: Box::new(TypeRef::Float),
                        },
                        summary: "Parse a floating-point number from bytes selected by an exclusive range in little-endian byte order.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Exclusive byte range to parse as a floating-point value.",
                        }],
                        examples: &[
                            "let value = blob(8, 0).parse_le_float(0..8);",
                            "// value parses the selected bytes as a little-endian float",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive],
                            ret: Box::new(TypeRef::Float),
                        },
                        summary: "Parse a floating-point number from bytes selected by an inclusive range in little-endian byte order.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Inclusive byte range to parse as a floating-point value.",
                        }],
                        examples: &[
                            "let value = blob(8, 0).parse_le_float(0..=7);",
                            "// value parses the selected bytes as a little-endian float",
                        ],
                    },
                ],
            ),
            blob_overloaded_method(
                "parse_be_float",
                "Parse a floating-point number from the BLOB in big-endian byte order.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int],
                            ret: Box::new(TypeRef::Float),
                        },
                        summary: "Parse a floating-point number from a fixed byte window in big-endian byte order.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Byte offset where parsing begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of bytes to interpret as a floating-point value.",
                            },
                        ],
                        examples: &[
                            "let value = blob(8, 0).parse_be_float(0, 8);",
                            "// value parses the bytes as a big-endian float",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range],
                            ret: Box::new(TypeRef::Float),
                        },
                        summary: "Parse a floating-point number from bytes selected by an exclusive range in big-endian byte order.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Exclusive byte range to parse as a floating-point value.",
                        }],
                        examples: &[
                            "let value = blob(8, 0).parse_be_float(0..8);",
                            "// value parses the selected bytes as a big-endian float",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive],
                            ret: Box::new(TypeRef::Float),
                        },
                        summary: "Parse a floating-point number from bytes selected by an inclusive range in big-endian byte order.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Inclusive byte range to parse as a floating-point value.",
                        }],
                        examples: &[
                            "let value = blob(8, 0).parse_be_float(0..=7);",
                            "// value parses the selected bytes as a big-endian float",
                        ],
                    },
                ],
            ),
            blob_overloaded_method(
                "write_le",
                "Write an integer or floating-point value into the BLOB in little-endian byte order.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int, TypeRef::Any],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Write an integer or floating-point value into a fixed byte window in little-endian byte order.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Byte offset where writing begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of bytes reserved for the encoded value.",
                            },
                            BuiltinParamDoc {
                                name: "value",
                                description: "Integer or floating-point value encoded into the selected bytes.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(4, 0);",
                            "buf.write_le(0, 4, 42);",
                            "// buf now stores 42 in little-endian byte order",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range, TypeRef::Any],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Write an integer or floating-point value into bytes selected by an exclusive range in little-endian byte order.",
                        params: &[
                            BuiltinParamDoc {
                                name: "range",
                                description: "Exclusive byte range that receives the encoded value.",
                            },
                            BuiltinParamDoc {
                                name: "value",
                                description: "Integer or floating-point value encoded into the selected bytes.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(4, 0);",
                            "buf.write_le(0..4, 42);",
                            "// buf now stores 42 in little-endian byte order",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive, TypeRef::Any],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Write an integer or floating-point value into bytes selected by an inclusive range in little-endian byte order.",
                        params: &[
                            BuiltinParamDoc {
                                name: "range",
                                description: "Inclusive byte range that receives the encoded value.",
                            },
                            BuiltinParamDoc {
                                name: "value",
                                description: "Integer or floating-point value encoded into the selected bytes.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(4, 0);",
                            "buf.write_le(0..=3, 42);",
                            "// buf now stores 42 in little-endian byte order",
                        ],
                    },
                ],
            ),
            blob_overloaded_method(
                "write_be",
                "Write an integer or floating-point value into the BLOB in big-endian byte order.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int, TypeRef::Any],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Write an integer or floating-point value into a fixed byte window in big-endian byte order.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Byte offset where writing begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of bytes reserved for the encoded value.",
                            },
                            BuiltinParamDoc {
                                name: "value",
                                description: "Integer or floating-point value encoded into the selected bytes.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(4, 0);",
                            "buf.write_be(0, 4, 42);",
                            "// buf now stores 42 in big-endian byte order",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range, TypeRef::Any],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Write an integer or floating-point value into bytes selected by an exclusive range in big-endian byte order.",
                        params: &[
                            BuiltinParamDoc {
                                name: "range",
                                description: "Exclusive byte range that receives the encoded value.",
                            },
                            BuiltinParamDoc {
                                name: "value",
                                description: "Integer or floating-point value encoded into the selected bytes.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(4, 0);",
                            "buf.write_be(0..4, 42);",
                            "// buf now stores 42 in big-endian byte order",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive, TypeRef::Any],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Write an integer or floating-point value into bytes selected by an inclusive range in big-endian byte order.",
                        params: &[
                            BuiltinParamDoc {
                                name: "range",
                                description: "Inclusive byte range that receives the encoded value.",
                            },
                            BuiltinParamDoc {
                                name: "value",
                                description: "Integer or floating-point value encoded into the selected bytes.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(4, 0);",
                            "buf.write_be(0..=3, 42);",
                            "// buf now stores 42 in big-endian byte order",
                        ],
                    },
                ],
            ),
            blob_overloaded_method(
                "write_utf8",
                "Write a string into the BLOB in UTF-8 encoding.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int, TypeRef::String],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Write a string into a fixed byte window using UTF-8 encoding.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Byte offset where writing begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of bytes available for the encoded string.",
                            },
                            BuiltinParamDoc {
                                name: "text",
                                description: "String encoded as UTF-8 into the selected bytes.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(6, 0);",
                            "buf.write_utf8(1, 4, \"Rhai\");",
                            "// buf contains the UTF-8 bytes for \"Rhai\" in the selected window",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range, TypeRef::String],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Write a string into bytes selected by an exclusive range using UTF-8 encoding.",
                        params: &[
                            BuiltinParamDoc {
                                name: "range",
                                description: "Exclusive byte range that receives the encoded string.",
                            },
                            BuiltinParamDoc {
                                name: "text",
                                description: "String encoded as UTF-8 into the selected bytes.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(6, 0);",
                            "buf.write_utf8(1..5, \"Rhai\");",
                            "// buf contains the UTF-8 bytes for \"Rhai\" in the selected range",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive, TypeRef::String],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Write a string into bytes selected by an inclusive range using UTF-8 encoding.",
                        params: &[
                            BuiltinParamDoc {
                                name: "range",
                                description: "Inclusive byte range that receives the encoded string.",
                            },
                            BuiltinParamDoc {
                                name: "text",
                                description: "String encoded as UTF-8 into the selected bytes.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(6, 0);",
                            "buf.write_utf8(1..=4, \"Rhai\");",
                            "// buf contains the UTF-8 bytes for \"Rhai\" in the selected range",
                        ],
                    },
                ],
            ),
            blob_overloaded_method(
                "write_ascii",
                "Write a string into the BLOB in 7-bit ASCII encoding.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int, TypeRef::String],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Write a string into a fixed byte window using 7-bit ASCII encoding.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Byte offset where writing begins.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of bytes available for the encoded string.",
                            },
                            BuiltinParamDoc {
                                name: "text",
                                description: "String encoded as ASCII into the selected bytes.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(6, 0);",
                            "buf.write_ascii(1, 4, \"Rhai\");",
                            "// buf contains the ASCII bytes for \"Rhai\" in the selected window",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range, TypeRef::String],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Write a string into bytes selected by an exclusive range using 7-bit ASCII encoding.",
                        params: &[
                            BuiltinParamDoc {
                                name: "range",
                                description: "Exclusive byte range that receives the encoded string.",
                            },
                            BuiltinParamDoc {
                                name: "text",
                                description: "String encoded as ASCII into the selected bytes.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(6, 0);",
                            "buf.write_ascii(1..5, \"Rhai\");",
                            "// buf contains the ASCII bytes for \"Rhai\" in the selected range",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive, TypeRef::String],
                            ret: Box::new(TypeRef::Unit),
                        },
                        summary: "Write a string into bytes selected by an inclusive range using 7-bit ASCII encoding.",
                        params: &[
                            BuiltinParamDoc {
                                name: "range",
                                description: "Inclusive byte range that receives the encoded string.",
                            },
                            BuiltinParamDoc {
                                name: "text",
                                description: "String encoded as ASCII into the selected bytes.",
                            },
                        ],
                        examples: &[
                            "let buf = blob(6, 0);",
                            "buf.write_ascii(1..=4, \"Rhai\");",
                            "// buf contains the ASCII bytes for \"Rhai\" in the selected range",
                        ],
                    },
                ],
            ),
        ],
    }
}
