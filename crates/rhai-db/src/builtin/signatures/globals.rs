use rhai_hir::{ExternalSignatureIndex, FunctionTypeRef, TypeRef};

use crate::builtin::signatures::helpers::builtin_global_function;
use crate::types::HostFunction;

pub(crate) fn register_builtin_global_functions(
    external_signatures: &mut ExternalSignatureIndex,
) -> Vec<HostFunction> {
    register_builtin_external_signatures(external_signatures);

    vec![
        builtin_global_function(
            "blob",
            vec![
                FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Blob),
                },
                FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Blob),
                },
                FunctionTypeRef {
                    params: vec![TypeRef::Int, TypeRef::Int],
                    ret: Box::new(TypeRef::Blob),
                },
            ],
            "Create a new BLOB value, optionally with a specific length and initial byte value.",
            &[
                "let empty = blob();",
                "// empty == []",
                "let buffer = blob(4, 7);",
                "// buffer == [7, 7, 7, 7]",
                "let size = buffer.len();",
                "// size == 4",
            ],
            "https://rhai.rs/book/language/blobs.html",
        ),
        builtin_global_function(
            "timestamp",
            vec![FunctionTypeRef {
                params: Vec::new(),
                ret: Box::new(TypeRef::Timestamp),
            }],
            "Create a timestamp representing the current instant.",
            &[
                "let started = timestamp();",
                "// started is a timestamp value for the current moment",
                "let seconds = started.elapsed();",
                "// seconds is the elapsed duration in seconds",
            ],
            "https://rhai.rs/book/ref/timestamps.html",
        ),
        builtin_global_function(
            "Fn",
            vec![FunctionTypeRef {
                params: vec![TypeRef::String],
                ret: Box::new(TypeRef::FnPtr),
            }],
            "Create a function pointer from the name of a script or registered function.",
            &[
                "fn do_work(x) { x + 1 }",
                "let handler = Fn(\"do_work\");",
                "// handler is a function pointer to do_work",
                "let result = handler.call(41);",
                "// result == 42",
            ],
            "https://rhai.rs/book/language/fn-ptr.html",
        ),
        builtin_global_function(
            "is_def_var",
            vec![FunctionTypeRef {
                params: vec![TypeRef::String],
                ret: Box::new(TypeRef::Bool),
            }],
            "Check whether a variable is currently defined in the active scope.",
            &[
                "let config = #{ enabled: true };",
                "let exists = is_def_var(\"config\");",
                "// exists == true",
                "let missing = is_def_var(\"missing_value\");",
                "// missing == false",
            ],
            "https://rhai.rs/book/language/variables.html",
        ),
        builtin_global_function(
            "is_def_fn",
            vec![
                FunctionTypeRef {
                    params: vec![TypeRef::String, TypeRef::Int],
                    ret: Box::new(TypeRef::Bool),
                },
                FunctionTypeRef {
                    params: vec![TypeRef::String, TypeRef::String, TypeRef::Int],
                    ret: Box::new(TypeRef::Bool),
                },
            ],
            "Check whether a function or typed method is currently available.",
            &[
                "let has_render = is_def_fn(\"render\", 1);",
                "// has_render is true when a global render(value) function exists",
                "let has_open = is_def_fn(\"open\", \"Widget\", 1);",
                "// has_open is true when Widget.open(value) exists",
            ],
            "https://rhai.rs/book/language/fn-namespaces.html",
        ),
        builtin_global_function(
            "type_of",
            vec![FunctionTypeRef {
                params: vec![TypeRef::Any],
                ret: Box::new(TypeRef::String),
            }],
            "Return the dynamic Rhai type name for a value.",
            &[
                "let kind = type_of(42);",
                "// kind == \"i64\" or the engine's configured integer type name",
                "let record_kind = type_of(#{ name: \"Ada\" });",
                "// record_kind describes the value's dynamic Rhai type",
            ],
            "https://rhai.rs/book/language/type-of.html",
        ),
        builtin_global_function(
            "tag",
            vec![FunctionTypeRef {
                params: vec![TypeRef::Any],
                ret: Box::new(TypeRef::Int),
            }],
            "Return the dynamic value tag associated with a value.",
            &[
                "let value = 42;",
                "let current = tag(value);",
                "// current == 0 by default",
                "set_tag(value, 7);",
                "let tagged = tag(value);",
                "// tagged == 7",
            ],
            "https://rhai.rs/book/language/dynamic-tag.html",
        ),
        builtin_global_function(
            "set_tag",
            vec![FunctionTypeRef {
                params: vec![TypeRef::Any, TypeRef::Int],
                ret: Box::new(TypeRef::Unit),
            }],
            "Set the dynamic value tag for a value.",
            &[
                "let value = 42;",
                "set_tag(value, 99);",
                "let current = tag(value);",
                "// current == 99",
            ],
            "https://rhai.rs/book/language/dynamic-tag.html",
        ),
        builtin_global_function(
            "print",
            vec![FunctionTypeRef {
                params: vec![TypeRef::Any],
                ret: Box::new(TypeRef::Unit),
            }],
            "Print a value through the engine's standard print callback.",
            &[
                "print(\"hello from Rhai\");",
                "// writes through the engine's print callback",
                "print(#{ status: \"ok\", code: 200 });",
                "// object maps are formatted with the active print callback",
            ],
            "https://rhai.rs/book/start/hello-world.html",
        ),
        builtin_global_function(
            "debug",
            vec![FunctionTypeRef {
                params: vec![TypeRef::Any],
                ret: Box::new(TypeRef::Unit),
            }],
            "Print a value through the engine's debug callback.",
            &[
                "debug(#{ user: \"Ada\" });",
                "// writes a debug representation through the engine's debug callback",
                "debug([1, 2, 3]);",
                "// useful while inspecting values during script development",
            ],
            "https://rhai.rs/book/rust/engine.html",
        ),
        builtin_global_function(
            "parse_int",
            vec![
                FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::Int),
                },
                FunctionTypeRef {
                    params: vec![TypeRef::String, TypeRef::Int],
                    ret: Box::new(TypeRef::Int),
                },
            ],
            "Parse a string into an integer value, optionally using a custom radix.",
            &[
                "let answer = parse_int(\"42\");",
                "// answer == 42",
                "let hex = parse_int(\"ff\", 16);",
                "// hex == 255",
                "let binary = parse_int(\"1010\", 2);",
                "// binary == 10",
            ],
            "https://rhai.rs/book/language/num-fn.html",
        ),
        builtin_global_function(
            "parse_float",
            vec![FunctionTypeRef {
                params: vec![TypeRef::String],
                ret: Box::new(TypeRef::Float),
            }],
            "Parse a string into a floating-point number.",
            &[
                "let ratio = parse_float(\"3.14159\");",
                "// ratio == 3.14159",
                "let negative = parse_float(\"-2.5\");",
                "// negative == -2.5",
            ],
            "https://rhai.rs/book/language/num-fn.html",
        ),
        builtin_global_function(
            "parse_decimal",
            vec![FunctionTypeRef {
                params: vec![TypeRef::String],
                ret: Box::new(TypeRef::Decimal),
            }],
            "Parse a string into a fixed-precision decimal number.",
            &[
                "let amount = parse_decimal(\"12.34\");",
                "// amount == 12.34 as a decimal value",
            ],
            "https://rhai.rs/book/language/num-fn.html",
        ),
        builtin_global_function(
            "eval",
            vec![FunctionTypeRef {
                params: vec![TypeRef::String],
                ret: Box::new(TypeRef::Dynamic),
            }],
            "Evaluate a script string in the current scope and return its result.",
            &[
                "let answer = eval(\"40 + 2\");",
                "// answer == 42",
                "let base = 40;",
                "let total = eval(\"base + 2\");",
                "// total == 42",
            ],
            "https://rhai.rs/book/language/eval.html",
        ),
        builtin_global_function(
            "sleep",
            vec![FunctionTypeRef {
                params: vec![TypeRef::Union(vec![TypeRef::Int, TypeRef::Float])],
                ret: Box::new(TypeRef::Unit),
            }],
            "Block the current thread for the specified number of seconds.",
            &[
                "sleep(0.5);",
                "// pauses the current script for half a second",
                "sleep(1);",
                "// pauses the current script for one second",
            ],
            "https://rhai.rs/book/ref/timestamps.html",
        ),
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
        "tag",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Any],
            ret: Box::new(TypeRef::Int),
        }),
    );
    external_signatures.insert(
        "set_tag",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Any, TypeRef::Int],
            ret: Box::new(TypeRef::Unit),
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
        "parse_decimal",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::String],
            ret: Box::new(TypeRef::Decimal),
        }),
    );
    external_signatures.insert(
        "eval",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::String],
            ret: Box::new(TypeRef::Dynamic),
        }),
    );
    external_signatures.insert(
        "sleep",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Union(vec![TypeRef::Int, TypeRef::Float])],
            ret: Box::new(TypeRef::Unit),
        }),
    );
}
