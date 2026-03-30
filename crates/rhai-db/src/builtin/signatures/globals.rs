use rhai_hir::{ExternalSignatureIndex, FunctionTypeRef, TypeRef};

use crate::types::{HostFunction, HostFunctionOverload};

pub(crate) fn register_builtin_global_functions(
    external_signatures: &mut ExternalSignatureIndex,
) -> Vec<HostFunction> {
    register_builtin_external_signatures(external_signatures);

    vec![
        HostFunction {
            name: "blob".to_owned(),
            overloads: vec![
                HostFunctionOverload {
                    signature: Some(FunctionTypeRef {
                        params: Vec::new(),
                        ret: Box::new(TypeRef::Blob),
                    }),
                    docs: Some("Create an empty BLOB.".to_owned()),
                },
                HostFunctionOverload {
                    signature: Some(FunctionTypeRef {
                        params: vec![TypeRef::Int],
                        ret: Box::new(TypeRef::Blob),
                    }),
                    docs: Some("Create a BLOB with the given length.".to_owned()),
                },
                HostFunctionOverload {
                    signature: Some(FunctionTypeRef {
                        params: vec![TypeRef::Int, TypeRef::Int],
                        ret: Box::new(TypeRef::Blob),
                    }),
                    docs: Some("Create a BLOB filled with the given byte value.".to_owned()),
                },
            ],
        },
        HostFunction {
            name: "timestamp".to_owned(),
            overloads: vec![HostFunctionOverload {
                signature: Some(FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Timestamp),
                }),
                docs: Some("Create a timestamp for the current instant.".to_owned()),
            }],
        },
        HostFunction {
            name: "Fn".to_owned(),
            overloads: vec![HostFunctionOverload {
                signature: Some(FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::FnPtr),
                }),
                docs: Some("Create a function pointer from a function name.".to_owned()),
            }],
        },
        HostFunction {
            name: "is_def_var".to_owned(),
            overloads: vec![HostFunctionOverload {
                signature: Some(FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::Bool),
                }),
                docs: Some("Check whether a variable is defined.".to_owned()),
            }],
        },
        HostFunction {
            name: "is_def_fn".to_owned(),
            overloads: vec![
                HostFunctionOverload {
                    signature: Some(FunctionTypeRef {
                        params: vec![TypeRef::String, TypeRef::Int],
                        ret: Box::new(TypeRef::Bool),
                    }),
                    docs: Some("Check whether a function is defined.".to_owned()),
                },
                HostFunctionOverload {
                    signature: Some(FunctionTypeRef {
                        params: vec![TypeRef::String, TypeRef::String, TypeRef::Int],
                        ret: Box::new(TypeRef::Bool),
                    }),
                    docs: Some("Check whether a typed method is defined.".to_owned()),
                },
            ],
        },
        HostFunction {
            name: "type_of".to_owned(),
            overloads: vec![HostFunctionOverload {
                signature: Some(FunctionTypeRef {
                    params: vec![TypeRef::Any],
                    ret: Box::new(TypeRef::String),
                }),
                docs: Some("Get the type name of a value.".to_owned()),
            }],
        },
        HostFunction {
            name: "print".to_owned(),
            overloads: vec![HostFunctionOverload {
                signature: Some(FunctionTypeRef {
                    params: vec![TypeRef::Any],
                    ret: Box::new(TypeRef::Unit),
                }),
                docs: Some("Print a value via the engine's print callback.".to_owned()),
            }],
        },
        HostFunction {
            name: "debug".to_owned(),
            overloads: vec![HostFunctionOverload {
                signature: Some(FunctionTypeRef {
                    params: vec![TypeRef::Any],
                    ret: Box::new(TypeRef::Unit),
                }),
                docs: Some("Print a value via the engine's debug callback.".to_owned()),
            }],
        },
        HostFunction {
            name: "parse_int".to_owned(),
            overloads: vec![
                HostFunctionOverload {
                    signature: Some(FunctionTypeRef {
                        params: vec![TypeRef::String],
                        ret: Box::new(TypeRef::Int),
                    }),
                    docs: Some("Convert a string into an integer.".to_owned()),
                },
                HostFunctionOverload {
                    signature: Some(FunctionTypeRef {
                        params: vec![TypeRef::String, TypeRef::Int],
                        ret: Box::new(TypeRef::Int),
                    }),
                    docs: Some(
                        "Convert a string into an integer using the given radix.".to_owned(),
                    ),
                },
            ],
        },
        HostFunction {
            name: "parse_float".to_owned(),
            overloads: vec![HostFunctionOverload {
                signature: Some(FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::Float),
                }),
                docs: Some("Convert a string into a floating-point number.".to_owned()),
            }],
        },
        HostFunction {
            name: "eval".to_owned(),
            overloads: vec![HostFunctionOverload {
                signature: Some(FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::Dynamic),
                }),
                docs: Some(
                    "Evaluate a script string inside the current scope and return its result."
                        .to_owned(),
                ),
            }],
        },
    ]
}

fn register_builtin_external_signatures(external_signatures: &mut ExternalSignatureIndex) {
    external_signatures.insert(
        "blob",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Int],
            ret: Box::new(TypeRef::Blob),
        }),
    );
    external_signatures.insert(
        "timestamp",
        TypeRef::Function(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Timestamp),
        }),
    );
    external_signatures.insert(
        "Fn",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::String],
            ret: Box::new(TypeRef::FnPtr),
        }),
    );
    external_signatures.insert(
        "is_def_var",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::String],
            ret: Box::new(TypeRef::Bool),
        }),
    );
    external_signatures.insert(
        "is_def_fn",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::String, TypeRef::Int],
            ret: Box::new(TypeRef::Bool),
        }),
    );
    external_signatures.insert(
        "type_of",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Any],
            ret: Box::new(TypeRef::String),
        }),
    );
    external_signatures.insert(
        "print",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Any],
            ret: Box::new(TypeRef::Unit),
        }),
    );
    external_signatures.insert(
        "debug",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Any],
            ret: Box::new(TypeRef::Unit),
        }),
    );
    external_signatures.insert(
        "parse_int",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::String],
            ret: Box::new(TypeRef::Int),
        }),
    );
    external_signatures.insert(
        "parse_float",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::String],
            ret: Box::new(TypeRef::Float),
        }),
    );
    external_signatures.insert(
        "eval",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::String],
            ret: Box::new(TypeRef::Dynamic),
        }),
    );
}
