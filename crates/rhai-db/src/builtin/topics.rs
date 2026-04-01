use crate::builtin::signatures::builtin_topic_docs;
use rhai_hir::{AssignmentOperator, BinaryOperator, TypeRef, UnaryOperator};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinTopicKey {
    ArrayIndex,
    ArrayRangeIndex,
    BlobIndex,
    BlobRangeIndex,
    StringIndex,
    StringRangeIndex,
    MapIndex,
    MapPropertyAccess,
    IntBitIndex,
    IntBitRangeIndex,
    ContainsOperator,
    RangeOperator,
    RangeInclusiveOperator,
    NumericAdditionOperator,
    NumericArithmeticOperator,
    EqualityOperator,
    ComparisonOperator,
    StringConcatenationOperator,
    ArrayConcatenationOperator,
    BlobConcatenationOperator,
    MapMergeOperator,
    NullCoalesceOperator,
    NumericAssignmentOperator,
    StringAppendAssignmentOperator,
    ArrayAppendAssignmentOperator,
    BlobAppendAssignmentOperator,
    BitwiseAssignmentOperator,
    NullCoalesceAssignmentOperator,
    UnaryPlusOperator,
    UnaryMinusOperator,
    LogicalNotOperator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinTopicDoc {
    pub key: BuiltinTopicKey,
    pub signature: String,
    pub docs: String,
    pub notes: Vec<String>,
}

pub fn builtin_indexer_topic(
    receiver_ty: &TypeRef,
    index_ty: Option<&TypeRef>,
) -> Option<BuiltinTopicDoc> {
    match receiver_ty {
        TypeRef::Array(_) => {
            if is_range_index(index_ty) {
                Some(array_range_index_topic())
            } else {
                Some(array_index_topic())
            }
        }
        TypeRef::Blob => {
            if is_range_index(index_ty) {
                Some(blob_range_index_topic())
            } else {
                Some(blob_index_topic())
            }
        }
        TypeRef::String => {
            if is_range_index(index_ty) {
                Some(string_range_index_topic())
            } else {
                Some(string_index_topic())
            }
        }
        TypeRef::Map(_, _) | TypeRef::Object(_) => Some(map_index_topic()),
        TypeRef::Int => {
            if is_range_index(index_ty) {
                Some(int_bit_range_index_topic())
            } else if is_int_index(index_ty) {
                Some(int_bit_index_topic())
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn builtin_property_access_topic(receiver_ty: &TypeRef) -> Option<BuiltinTopicDoc> {
    match receiver_ty {
        TypeRef::Map(_, _) | TypeRef::Object(_) => Some(map_property_access_topic()),
        _ => None,
    }
}

pub fn builtin_binary_operator_topic(
    operator: BinaryOperator,
    lhs_ty: Option<&TypeRef>,
    rhs_ty: Option<&TypeRef>,
) -> Option<BuiltinTopicDoc> {
    match operator {
        BinaryOperator::In => contains_operator_topic(lhs_ty, rhs_ty),
        BinaryOperator::Range => Some(range_operator_topic()),
        BinaryOperator::RangeInclusive => Some(range_inclusive_operator_topic()),
        BinaryOperator::Add => additive_operator_topic(lhs_ty, rhs_ty),
        BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Remainder
        | BinaryOperator::Power
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Or
        | BinaryOperator::Xor
        | BinaryOperator::And => numeric_operator_topic(operator),
        BinaryOperator::EqEq | BinaryOperator::NotEq => equality_operator_topic(lhs_ty, rhs_ty),
        BinaryOperator::Gt | BinaryOperator::GtEq | BinaryOperator::Lt | BinaryOperator::LtEq => {
            comparison_operator_topic(lhs_ty, rhs_ty)
        }
        BinaryOperator::NullCoalesce => Some(null_coalesce_operator_topic()),
        BinaryOperator::OrOr | BinaryOperator::AndAnd => None,
    }
}

pub fn builtin_assignment_operator_topic(
    operator: AssignmentOperator,
    lhs_ty: Option<&TypeRef>,
    rhs_ty: Option<&TypeRef>,
) -> Option<BuiltinTopicDoc> {
    match operator {
        AssignmentOperator::Assign => None,
        AssignmentOperator::NullCoalesce => Some(null_coalesce_assignment_topic()),
        AssignmentOperator::Add => additive_assignment_topic(lhs_ty, rhs_ty),
        AssignmentOperator::Subtract
        | AssignmentOperator::Multiply
        | AssignmentOperator::Divide
        | AssignmentOperator::Remainder
        | AssignmentOperator::Power => Some(numeric_assignment_topic(operator)),
        AssignmentOperator::ShiftLeft
        | AssignmentOperator::ShiftRight
        | AssignmentOperator::Or
        | AssignmentOperator::Xor
        | AssignmentOperator::And => Some(bitwise_assignment_topic(operator)),
    }
}

pub fn builtin_unary_operator_topic(
    operator: UnaryOperator,
    operand_ty: Option<&TypeRef>,
) -> Option<BuiltinTopicDoc> {
    match operator {
        UnaryOperator::Plus => unary_plus_operator_topic(operand_ty),
        UnaryOperator::Minus => unary_minus_operator_topic(operand_ty),
        UnaryOperator::Not => logical_not_operator_topic(operand_ty),
    }
}

fn array_index_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::ArrayIndex,
        signature: "array[index] -> any".to_owned(),
        docs: builtin_topic_docs(
            "Index an array by zero-based position to read or write a single element. Negative indices count from the end of the array.",
            &["let value = items[0];", "items[1] = 42;"],
            &[
                "let items = [10, 20, 30];",
                "let first = items[0];",
                "// first == 10",
                "items[-1] = 99;",
                "// items == [10, 20, 99]",
            ],
            "https://rhai.rs/book/ref/arrays.html",
        ),
        notes: vec!["Builtin array indexing syntax.".to_owned()],
    }
}

fn array_range_index_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::ArrayRangeIndex,
        signature: "array[range] -> array<any>".to_owned(),
        docs: builtin_topic_docs(
            "Slice an array with a range to produce a new array containing the selected elements.",
            &["let part = items[1..3];", "items[1..=2] = [7, 8];"],
            &[
                "let items = [10, 20, 30, 40];",
                "let middle = items[1..3];",
                "// middle == [20, 30]",
                "items[1..=2] = [7, 8];",
                "// items == [10, 7, 8, 40]",
            ],
            "https://rhai.rs/book/ref/arrays.html",
        ),
        notes: vec!["Builtin array slicing syntax.".to_owned()],
    }
}

fn blob_index_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::BlobIndex,
        signature: "blob[index] -> int".to_owned(),
        docs: builtin_topic_docs(
            "Index a BLOB by byte position to read or write a single byte. Negative indices count from the end of the BLOB.",
            &["let byte = buf[0];", "buf[1] = 255;"],
            &[
                "let buf = blob(3, 0);",
                "buf[1] = 255;",
                "// buf == [0, 255, 0]",
                "let byte = buf[-1];",
                "// byte == 0",
            ],
            "https://rhai.rs/book/language/blobs.html",
        ),
        notes: vec!["Builtin BLOB byte indexing syntax.".to_owned()],
    }
}

fn blob_range_index_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::BlobRangeIndex,
        signature: "blob[range] -> blob".to_owned(),
        docs: builtin_topic_docs(
            "Slice a BLOB with a range to produce a new BLOB containing the selected bytes.",
            &["let part = buf[1..3];", "buf[1..=2] = blob(2, 9);"],
            &[
                "let buf = blob(4, 7);",
                "let part = buf[1..3];",
                "// part contains the middle bytes",
                "buf[1..=2] = blob(2, 9);",
                "// buf now has 9 in the replaced positions",
            ],
            "https://rhai.rs/book/language/blobs.html",
        ),
        notes: vec!["Builtin BLOB slice indexing syntax.".to_owned()],
    }
}

fn string_index_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::StringIndex,
        signature: "string[index] -> char".to_owned(),
        docs: builtin_topic_docs(
            "Index a string by character position to read or write a single character. Negative indices count from the end of the string.",
            &["let ch = text[0];", "text[-1] = '!';"],
            &[
                "let text = \"Rhai\";",
                "let first = text[0];",
                "// first == 'R'",
                "text[-1] = '!';",
                "// text == \"Rha!\"",
            ],
            "https://rhai.rs/book/ref/strings-chars.html",
        ),
        notes: vec!["Builtin string character indexing syntax.".to_owned()],
    }
}

fn string_range_index_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::StringRangeIndex,
        signature: "string[range] -> string".to_owned(),
        docs: builtin_topic_docs(
            "Slice a string with a character range to produce a new string containing the selected characters.",
            &["let part = text[1..3];", "text[1..=2] = \"HA\";"],
            &[
                "let text = \"Rhai\";",
                "let part = text[1..3];",
                "// part == \"ha\"",
                "text[1..=2] = \"HA\";",
                "// text == \"RHAi\"",
            ],
            "https://rhai.rs/book/ref/strings-chars.html",
        ),
        notes: vec!["Builtin string slice indexing syntax.".to_owned()],
    }
}

fn map_index_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::MapIndex,
        signature: "map[\"field\"] -> any | ()".to_owned(),
        docs: builtin_topic_docs(
            "Index an object map by property name to read or write a field. Missing fields read as `()` until written. This is the bracket form of object-map property access and is interchangeable with dot syntax when the field name is a valid identifier.",
            &[
                "let value = record[\"name\"];",
                "record[\"active\"] = true;",
                "let same = record.name;",
            ],
            &[
                "let user = #{ name: \"Ada\" };",
                "let name = user[\"name\"];",
                "// name == \"Ada\"",
                "user[\"active\"] = true;",
                "// user == #{ name: \"Ada\", active: true }",
                "let same = user.name;",
                "// same == \"Ada\"",
            ],
            "https://rhai.rs/book/ref/object-maps.html",
        ),
        notes: vec![
            "Builtin object map indexing syntax.".to_owned(),
            "Equivalent property syntax: map.field when the field name is a valid identifier."
                .to_owned(),
        ],
    }
}

fn map_property_access_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::MapPropertyAccess,
        signature: "map.field -> any | ()".to_owned(),
        docs: builtin_topic_docs(
            "Access an object map property by name using dot syntax. Missing fields read as `()` until written. This is interchangeable with bracket indexing when the field name is a valid identifier.",
            &[
                "let value = record.name;",
                "record.active = true;",
                "let same = record[\"name\"];",
            ],
            &[
                "let user = #{ name: \"Ada\" };",
                "let name = user.name;",
                "// name == \"Ada\"",
                "user.active = true;",
                "// user == #{ name: \"Ada\", active: true }",
                "let same = user[\"name\"];",
                "// same == \"Ada\"",
            ],
            "https://rhai.rs/book/ref/object-maps.html",
        ),
        notes: vec![
            "Builtin object map property access syntax.".to_owned(),
            "Equivalent indexing syntax: map[\"field\"] for dynamic or quoted property access."
                .to_owned(),
        ],
    }
}

fn int_bit_index_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::IntBitIndex,
        signature: "int[index] -> bool".to_owned(),
        docs: builtin_topic_docs(
            "Index an integer as a bit-field to read or write a single bit. Non-negative indices count from the least-significant bit, while negative indices count from the most-significant bit.",
            &["let flag = value[3];", "value[3] = true;"],
            &[
                "let value = 0b1010;",
                "let flag = value[1];",
                "// flag == true",
                "let high = value[-1];",
                "// high reads from the most-significant side",
                "value[0] = true;",
                "// value now has bit 0 enabled",
            ],
            "https://rhai.rs/book/language/bit-fields.html",
        ),
        notes: vec!["Builtin integer bit-field indexing syntax.".to_owned()],
    }
}

fn int_bit_range_index_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::IntBitRangeIndex,
        signature: "int[range] -> int".to_owned(),
        docs: builtin_topic_docs(
            "Slice an integer bit-field with a range to extract or replace a group of bits. Range indices count from the least-significant bit.",
            &["let data = value[4..=11];", "value[4..=11] = 0b1010;"],
            &[
                "let value = 0b110110;",
                "let middle = value[1..4];",
                "// middle == 0b011",
                "value[1..=3] = 0b000;",
                "// value now has the selected bits cleared",
            ],
            "https://rhai.rs/book/language/bit-fields.html",
        ),
        notes: vec!["Builtin integer bit-field range indexing syntax.".to_owned()],
    }
}

fn contains_operator_topic(
    _lhs_ty: Option<&TypeRef>,
    rhs_ty: Option<&TypeRef>,
) -> Option<BuiltinTopicDoc> {
    let rhs = rhs_ty?;
    let signature = match rhs {
        TypeRef::Array(_) => "value in array -> bool",
        TypeRef::String => "char in string -> bool",
        TypeRef::Blob => "byte in blob -> bool",
        TypeRef::Map(_, _) | TypeRef::Object(_) => "string in map -> bool",
        TypeRef::Range | TypeRef::RangeInclusive => "int in range -> bool",
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => {
            let resolved = members.iter().find(|member| {
                matches!(
                    member,
                    TypeRef::Array(_)
                        | TypeRef::String
                        | TypeRef::Blob
                        | TypeRef::Map(_, _)
                        | TypeRef::Object(_)
                        | TypeRef::Range
                        | TypeRef::RangeInclusive
                )
            })?;
            return contains_operator_topic(None, Some(resolved));
        }
        _ => return None,
    };

    Some(BuiltinTopicDoc {
        key: BuiltinTopicKey::ContainsOperator,
        signature: signature.to_owned(),
        docs: builtin_topic_docs(
            "Check whether the right-hand collection, map, or range contains the left-hand value.",
            &["let found = value in collection;"],
            &[
                "let ok = 2 in [1, 2, 3];",
                "// ok == true",
                "let has_name = \"name\" in #{ name: \"Ada\" };",
                "// has_name == true",
                "let inside = 3 in 1..=5;",
                "// inside == true",
            ],
            "https://rhai.rs/book/language/expressions.html",
        ),
        notes: vec!["Builtin membership operator.".to_owned()],
    })
}

fn additive_operator_topic(
    lhs_ty: Option<&TypeRef>,
    rhs_ty: Option<&TypeRef>,
) -> Option<BuiltinTopicDoc> {
    let ty = dominant_operator_type(lhs_ty, rhs_ty)?;

    match ty {
        TypeRef::String | TypeRef::Char => Some(string_concatenation_operator_topic()),
        TypeRef::Array(_) => Some(array_concatenation_operator_topic()),
        TypeRef::Blob => Some(blob_concatenation_operator_topic()),
        TypeRef::Map(_, _) | TypeRef::Object(_) => Some(map_merge_operator_topic()),
        TypeRef::Int | TypeRef::Float | TypeRef::Decimal => Some(numeric_addition_operator_topic()),
        _ => None,
    }
}

fn numeric_operator_topic(operator: BinaryOperator) -> Option<BuiltinTopicDoc> {
    let (signature, summary, usage, examples): (&str, &str, &[&str], &[&str]) = match operator {
        BinaryOperator::Subtract => (
            "number - number -> number",
            "Subtract the right-hand numeric value from the left-hand numeric value.",
            &["let diff = 10 - 3;"][..],
            &["let diff = 10 - 3;", "// diff == 7"][..],
        ),
        BinaryOperator::Multiply => (
            "number * number -> number",
            "Multiply two numeric values.",
            &["let product = 6 * 7;"][..],
            &["let product = 6 * 7;", "// product == 42"][..],
        ),
        BinaryOperator::Divide => (
            "number / number -> number",
            "Divide one numeric value by another.",
            &["let ratio = 21 / 3;"][..],
            &["let ratio = 21 / 3;", "// ratio == 7"][..],
        ),
        BinaryOperator::Remainder => (
            "number % number -> number",
            "Compute the remainder left after division.",
            &["let remainder = 10 % 4;"][..],
            &["let remainder = 10 % 4;", "// remainder == 2"][..],
        ),
        BinaryOperator::Power => (
            "number ** number -> number",
            "Raise the left-hand numeric value to the power of the right-hand numeric value.",
            &["let result = 2 ** 3;"][..],
            &["let result = 2 ** 3;", "// result == 8"][..],
        ),
        BinaryOperator::ShiftLeft => (
            "int << int -> int",
            "Shift an integer left by the specified number of bits.",
            &["let shifted = 1 << 3;"][..],
            &["let shifted = 1 << 3;", "// shifted == 8"][..],
        ),
        BinaryOperator::ShiftRight => (
            "int >> int -> int",
            "Shift an integer right by the specified number of bits.",
            &["let shifted = 16 >> 2;"][..],
            &["let shifted = 16 >> 2;", "// shifted == 4"][..],
        ),
        BinaryOperator::Or => (
            "int | int -> int",
            "Combine two integers with bitwise OR.",
            &["let flags = 0b0101 | 0b0011;"][..],
            &["let flags = 0b0101 | 0b0011;", "// flags == 0b0111"][..],
        ),
        BinaryOperator::Xor => (
            "int ^ int -> int",
            "Combine two integers with bitwise XOR.",
            &["let flags = 0b0101 ^ 0b0011;"][..],
            &["let flags = 0b0101 ^ 0b0011;", "// flags == 0b0110"][..],
        ),
        BinaryOperator::And => (
            "int & int -> int",
            "Combine two integers with bitwise AND.",
            &["let flags = 0b0101 & 0b0011;"][..],
            &["let flags = 0b0101 & 0b0011;", "// flags == 0b0001"][..],
        ),
        _ => return None,
    };

    Some(BuiltinTopicDoc {
        key: BuiltinTopicKey::NumericArithmeticOperator,
        signature: signature.to_owned(),
        docs: builtin_topic_docs(
            summary,
            usage,
            examples,
            "https://rhai.rs/book/ref/numbers.html",
        ),
        notes: vec!["Builtin numeric operator.".to_owned()],
    })
}

fn equality_operator_topic(
    lhs_ty: Option<&TypeRef>,
    rhs_ty: Option<&TypeRef>,
) -> Option<BuiltinTopicDoc> {
    let ty = dominant_operator_type(lhs_ty, rhs_ty)?;
    let (signature, summary, examples, reference_url): (&str, &str, &[&str], &str) = match ty {
        TypeRef::String | TypeRef::Char => (
            "string == string -> bool",
            "Compare strings or characters for equality or inequality.",
            &[
                "let same = \"Ada\" == \"Ada\";",
                "// same == true",
                "let different = 'a' != 'b';",
                "// different == true",
            ][..],
            "https://rhai.rs/book/ref/strings-chars.html",
        ),
        TypeRef::Int | TypeRef::Float | TypeRef::Decimal | TypeRef::Bool => (
            "value == value -> bool",
            "Compare builtin scalar values for equality or inequality.",
            &[
                "let same = 2 == 2;",
                "// same == true",
                "let different = 2 != 3;",
                "// different == true",
            ][..],
            "https://rhai.rs/book/ref/numbers.html",
        ),
        TypeRef::Array(_) | TypeRef::Blob | TypeRef::Map(_, _) | TypeRef::Object(_) => (
            "value == value -> bool",
            "Compare builtin container values for equality or inequality.",
            &[
                "let same = [1, 2] == [1, 2];",
                "// same == true",
                "let changed = #{ a: 1 } != #{ a: 2 };",
                "// changed == true",
            ][..],
            "https://rhai.rs/book/language/expressions.html",
        ),
        _ => return None,
    };

    Some(BuiltinTopicDoc {
        key: BuiltinTopicKey::EqualityOperator,
        signature: signature.to_owned(),
        docs: builtin_topic_docs(
            summary,
            &[
                "let same = left == right;",
                "let different = left != right;",
            ],
            examples,
            reference_url,
        ),
        notes: vec!["Builtin equality operator.".to_owned()],
    })
}

fn comparison_operator_topic(
    lhs_ty: Option<&TypeRef>,
    rhs_ty: Option<&TypeRef>,
) -> Option<BuiltinTopicDoc> {
    let ty = dominant_operator_type(lhs_ty, rhs_ty)?;
    let (signature, summary, examples, reference_url): (&str, &str, &[&str], &str) = match ty {
        TypeRef::String | TypeRef::Char => (
            "string < string -> bool",
            "Compare strings or characters using lexical ordering.",
            &[
                "let ordered = \"alpha\" < \"beta\";",
                "// ordered == true",
                "let after = 'z' > 'a';",
                "// after == true",
            ][..],
            "https://rhai.rs/book/ref/strings-chars.html",
        ),
        TypeRef::Int | TypeRef::Float | TypeRef::Decimal => (
            "number < number -> bool",
            "Compare numeric values using ordering operators.",
            &[
                "let ordered = 2 < 5;",
                "// ordered == true",
                "let at_least = 3 >= 3;",
                "// at_least == true",
            ][..],
            "https://rhai.rs/book/ref/numbers.html",
        ),
        _ => return None,
    };

    Some(BuiltinTopicDoc {
        key: BuiltinTopicKey::ComparisonOperator,
        signature: signature.to_owned(),
        docs: builtin_topic_docs(
            summary,
            &["let ordered = left < right;"],
            examples,
            reference_url,
        ),
        notes: vec!["Builtin comparison operator.".to_owned()],
    })
}

fn null_coalesce_operator_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::NullCoalesceOperator,
        signature: "value ?? fallback -> value".to_owned(),
        docs: builtin_topic_docs(
            "Return the left-hand value unless it is `()`, otherwise evaluate and return the fallback.",
            &["let result = maybe_name ?? \"Guest\";"],
            &[
                "let maybe_name = ();",
                "let result = maybe_name ?? \"Guest\";",
                "// result == \"Guest\"",
            ],
            "https://rhai.rs/book/language/expressions.html",
        ),
        notes: vec!["Builtin null-coalescing operator.".to_owned()],
    }
}

fn numeric_addition_operator_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::NumericAdditionOperator,
        signature: "number + number -> number".to_owned(),
        docs: builtin_topic_docs(
            "Add two numeric values together.",
            &["let total = 20 + 22;"],
            &["let total = 20 + 22;", "// total == 42"],
            "https://rhai.rs/book/ref/numbers.html",
        ),
        notes: vec!["Builtin numeric addition operator.".to_owned()],
    }
}

fn string_concatenation_operator_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::StringConcatenationOperator,
        signature: "string + string -> string".to_owned(),
        docs: builtin_topic_docs(
            "Concatenate strings or characters into a new string value.",
            &["let full = first + \" \" + last;"],
            &[
                "let full = \"Ada\" + \" Lovelace\";",
                "// full == \"Ada Lovelace\"",
            ],
            "https://rhai.rs/book/ref/strings-chars.html",
        ),
        notes: vec!["Builtin string concatenation operator.".to_owned()],
    }
}

fn array_concatenation_operator_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::ArrayConcatenationOperator,
        signature: "array + array -> array".to_owned(),
        docs: builtin_topic_docs(
            "Concatenate two arrays into a new array value.",
            &["let all = left + right;"],
            &["let all = [1, 2] + [3, 4];", "// all == [1, 2, 3, 4]"],
            "https://rhai.rs/book/ref/arrays.html",
        ),
        notes: vec!["Builtin array concatenation operator.".to_owned()],
    }
}

fn blob_concatenation_operator_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::BlobConcatenationOperator,
        signature: "blob + blob -> blob".to_owned(),
        docs: builtin_topic_docs(
            "Concatenate two BLOB values into a new byte array.",
            &["let merged = left + right;"],
            &[
                "let merged = blob(2, 1) + blob(2, 2);",
                "// merged contains the bytes from both blobs",
            ],
            "https://rhai.rs/book/language/blobs.html",
        ),
        notes: vec!["Builtin BLOB concatenation operator.".to_owned()],
    }
}

fn map_merge_operator_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::MapMergeOperator,
        signature: "map + map -> map".to_owned(),
        docs: builtin_topic_docs(
            "Merge object maps into a new object map value.",
            &["let merged = defaults + overrides;"],
            &[
                "let defaults = #{ retries: 3 };",
                "let merged = defaults + #{ retries: 5, debug: true };",
                "// merged == #{ retries: 5, debug: true }",
            ],
            "https://rhai.rs/book/ref/object-maps.html",
        ),
        notes: vec!["Builtin object-map merge operator.".to_owned()],
    }
}

fn additive_assignment_topic(
    lhs_ty: Option<&TypeRef>,
    rhs_ty: Option<&TypeRef>,
) -> Option<BuiltinTopicDoc> {
    let ty = dominant_operator_type(lhs_ty, rhs_ty)?;

    match ty {
        TypeRef::String | TypeRef::Char => Some(string_append_assignment_topic()),
        TypeRef::Array(_) => Some(array_append_assignment_topic()),
        TypeRef::Blob => Some(blob_append_assignment_topic()),
        TypeRef::Int | TypeRef::Float | TypeRef::Decimal => {
            Some(numeric_assignment_topic(AssignmentOperator::Add))
        }
        _ => None,
    }
}

fn numeric_assignment_topic(operator: AssignmentOperator) -> BuiltinTopicDoc {
    let (signature, summary, usage, examples): (&str, &str, &[&str], &[&str]) = match operator {
        AssignmentOperator::Add => (
            "number += number",
            "Add the right-hand numeric value into the left-hand variable in place.",
            &["total += 5;"][..],
            &["let total = 10;", "total += 5;", "// total == 15"][..],
        ),
        AssignmentOperator::Subtract => (
            "number -= number",
            "Subtract the right-hand numeric value from the left-hand variable in place.",
            &["count -= 1;"][..],
            &["let count = 3;", "count -= 1;", "// count == 2"][..],
        ),
        AssignmentOperator::Multiply => (
            "number *= number",
            "Multiply the left-hand numeric variable by the right-hand value in place.",
            &["score *= 2;"][..],
            &["let score = 7;", "score *= 2;", "// score == 14"][..],
        ),
        AssignmentOperator::Divide => (
            "number /= number",
            "Divide the left-hand numeric variable by the right-hand value in place.",
            &["ratio /= 2;"][..],
            &["let ratio = 8;", "ratio /= 2;", "// ratio == 4"][..],
        ),
        AssignmentOperator::Remainder => (
            "number %= number",
            "Update the left-hand numeric variable with the remainder after division.",
            &["remainder %= 3;"][..],
            &[
                "let remainder = 10;",
                "remainder %= 3;",
                "// remainder == 1",
            ][..],
        ),
        AssignmentOperator::Power => (
            "number **= number",
            "Raise the left-hand numeric variable to a power in place.",
            &["value **= 3;"][..],
            &["let value = 2;", "value **= 3;", "// value == 8"][..],
        ),
        _ => unreachable!(),
    };

    BuiltinTopicDoc {
        key: BuiltinTopicKey::NumericAssignmentOperator,
        signature: signature.to_owned(),
        docs: builtin_topic_docs(
            summary,
            usage,
            examples,
            "https://rhai.rs/book/ref/numbers.html",
        ),
        notes: vec!["Builtin numeric assignment operator.".to_owned()],
    }
}

fn string_append_assignment_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::StringAppendAssignmentOperator,
        signature: "string += string".to_owned(),
        docs: builtin_topic_docs(
            "Append characters or another string onto the left-hand string in place.",
            &["message += \"!\";"],
            &[
                "let message = \"Hi\";",
                "message += \"!\";",
                "// message == \"Hi!\"",
            ],
            "https://rhai.rs/book/ref/strings-chars.html",
        ),
        notes: vec!["Builtin string append assignment operator.".to_owned()],
    }
}

fn array_append_assignment_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::ArrayAppendAssignmentOperator,
        signature: "array += array".to_owned(),
        docs: builtin_topic_docs(
            "Append array elements onto the left-hand array in place.",
            &["values += [3, 4];"],
            &[
                "let values = [1, 2];",
                "values += [3, 4];",
                "// values == [1, 2, 3, 4]",
            ],
            "https://rhai.rs/book/ref/arrays.html",
        ),
        notes: vec!["Builtin array append assignment operator.".to_owned()],
    }
}

fn blob_append_assignment_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::BlobAppendAssignmentOperator,
        signature: "blob += blob".to_owned(),
        docs: builtin_topic_docs(
            "Append bytes from the right-hand BLOB onto the left-hand BLOB in place.",
            &["buf += blob(2, 9);"],
            &[
                "let buf = blob(2, 1);",
                "buf += blob(2, 9);",
                "// buf now contains the original and appended bytes",
            ],
            "https://rhai.rs/book/language/blobs.html",
        ),
        notes: vec!["Builtin BLOB append assignment operator.".to_owned()],
    }
}

fn bitwise_assignment_topic(operator: AssignmentOperator) -> BuiltinTopicDoc {
    let (signature, summary, usage, examples): (&str, &str, &[&str], &[&str]) = match operator {
        AssignmentOperator::ShiftLeft => (
            "int <<= int",
            "Shift the left-hand integer left by the specified number of bits in place.",
            &["flags <<= 1;"][..],
            &["let flags = 0b0011;", "flags <<= 1;", "// flags == 0b0110"][..],
        ),
        AssignmentOperator::ShiftRight => (
            "int >>= int",
            "Shift the left-hand integer right by the specified number of bits in place.",
            &["flags >>= 1;"][..],
            &["let flags = 0b0110;", "flags >>= 1;", "// flags == 0b0011"][..],
        ),
        AssignmentOperator::Or => (
            "int |= int",
            "Apply bitwise OR to the left-hand integer in place.",
            &["flags |= 0b0100;"][..],
            &[
                "let flags = 0b0001;",
                "flags |= 0b0100;",
                "// flags == 0b0101",
            ][..],
        ),
        AssignmentOperator::Xor => (
            "int ^= int",
            "Apply bitwise XOR to the left-hand integer in place.",
            &["flags ^= 0b0101;"][..],
            &[
                "let flags = 0b0111;",
                "flags ^= 0b0101;",
                "// flags == 0b0010",
            ][..],
        ),
        AssignmentOperator::And => (
            "int &= int",
            "Apply bitwise AND to the left-hand integer in place.",
            &["flags &= 0b0011;"][..],
            &[
                "let flags = 0b0111;",
                "flags &= 0b0011;",
                "// flags == 0b0011",
            ][..],
        ),
        _ => unreachable!(),
    };

    BuiltinTopicDoc {
        key: BuiltinTopicKey::BitwiseAssignmentOperator,
        signature: signature.to_owned(),
        docs: builtin_topic_docs(
            summary,
            usage,
            examples,
            "https://rhai.rs/book/language/bit-fields.html",
        ),
        notes: vec!["Builtin bitwise assignment operator.".to_owned()],
    }
}

fn null_coalesce_assignment_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::NullCoalesceAssignmentOperator,
        signature: "value ??= fallback".to_owned(),
        docs: builtin_topic_docs(
            "Assign the fallback only when the left-hand variable currently holds `()`.",
            &["name ??= \"Guest\";"],
            &[
                "let name = ();",
                "name ??= \"Guest\";",
                "// name == \"Guest\"",
            ],
            "https://rhai.rs/book/language/expressions.html",
        ),
        notes: vec!["Builtin null-coalescing assignment operator.".to_owned()],
    }
}

fn unary_plus_operator_topic(operand_ty: Option<&TypeRef>) -> Option<BuiltinTopicDoc> {
    let ty = operand_ty?;
    if !type_may_be_numeric(ty) {
        return None;
    }

    Some(BuiltinTopicDoc {
        key: BuiltinTopicKey::UnaryPlusOperator,
        signature: "+number -> number".to_owned(),
        docs: builtin_topic_docs(
            "Apply unary plus to a numeric value. This keeps the value unchanged but can make numeric intent explicit.",
            &["let copy = +value;"],
            &["let value = 42;", "let copy = +value;", "// copy == 42"],
            "https://rhai.rs/book/ref/numbers.html",
        ),
        notes: vec!["Builtin unary numeric operator.".to_owned()],
    })
}

fn unary_minus_operator_topic(operand_ty: Option<&TypeRef>) -> Option<BuiltinTopicDoc> {
    let ty = operand_ty?;
    if !type_may_be_numeric(ty) {
        return None;
    }

    Some(BuiltinTopicDoc {
        key: BuiltinTopicKey::UnaryMinusOperator,
        signature: "-number -> number".to_owned(),
        docs: builtin_topic_docs(
            "Negate a numeric value.",
            &["let delta = -value;"],
            &["let value = 42;", "let delta = -value;", "// delta == -42"],
            "https://rhai.rs/book/ref/numbers.html",
        ),
        notes: vec!["Builtin unary numeric negation operator.".to_owned()],
    })
}

fn logical_not_operator_topic(operand_ty: Option<&TypeRef>) -> Option<BuiltinTopicDoc> {
    let ty = operand_ty?;
    if !type_may_be_bool(ty) {
        return None;
    }

    Some(BuiltinTopicDoc {
        key: BuiltinTopicKey::LogicalNotOperator,
        signature: "!bool -> bool".to_owned(),
        docs: builtin_topic_docs(
            "Invert a boolean value.",
            &["let ready = !pending;"],
            &[
                "let pending = false;",
                "let ready = !pending;",
                "// ready == true",
            ],
            "https://rhai.rs/book/language/expressions.html",
        ),
        notes: vec!["Builtin logical negation operator.".to_owned()],
    })
}

fn range_operator_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::RangeOperator,
        signature: "start .. end -> range".to_owned(),
        docs: builtin_topic_docs(
            "Construct an exclusive range value. The end value is not included.",
            &["let range = 0..10;"],
            &[
                "let range = 0..5;",
                "// range contains 0, 1, 2, 3, 4",
                "for x in 0..3 { print(x); }",
                "// prints 0, then 1, then 2",
            ],
            "https://rhai.rs/book/ref/ranges.html",
        ),
        notes: vec!["Builtin exclusive range operator.".to_owned()],
    }
}

fn range_inclusive_operator_topic() -> BuiltinTopicDoc {
    BuiltinTopicDoc {
        key: BuiltinTopicKey::RangeInclusiveOperator,
        signature: "start ..= end -> range=".to_owned(),
        docs: builtin_topic_docs(
            "Construct an inclusive range value. The end value is included.",
            &["let range = 0..=10;"],
            &[
                "let range = 0..=3;",
                "// range contains 0, 1, 2, 3",
                "for x in 1..=2 { print(x); }",
                "// prints 1, then 2",
            ],
            "https://rhai.rs/book/ref/ranges.html",
        ),
        notes: vec!["Builtin inclusive range operator.".to_owned()],
    }
}

fn dominant_operator_type<'a>(
    lhs_ty: Option<&'a TypeRef>,
    rhs_ty: Option<&'a TypeRef>,
) -> Option<&'a TypeRef> {
    lhs_ty
        .filter(|ty| is_operator_topic_type(ty))
        .or_else(|| rhs_ty.filter(|ty| is_operator_topic_type(ty)))
}

fn is_operator_topic_type(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Int
        | TypeRef::Float
        | TypeRef::Decimal
        | TypeRef::String
        | TypeRef::Char
        | TypeRef::Blob
        | TypeRef::Map(_, _)
        | TypeRef::Object(_)
        | TypeRef::Array(_) => true,
        TypeRef::Nullable(inner) => is_operator_topic_type(inner),
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => {
            members.iter().any(is_operator_topic_type)
        }
        _ => false,
    }
}

fn type_may_be_numeric(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Int | TypeRef::Float | TypeRef::Decimal => true,
        TypeRef::Nullable(inner) => type_may_be_numeric(inner),
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => {
            members.iter().any(type_may_be_numeric)
        }
        _ => false,
    }
}

fn type_may_be_bool(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Bool => true,
        TypeRef::Nullable(inner) => type_may_be_bool(inner),
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => {
            members.iter().any(type_may_be_bool)
        }
        _ => false,
    }
}

fn is_int_index(index_ty: Option<&TypeRef>) -> bool {
    index_ty.is_none_or(type_may_be_int)
}

fn is_range_index(index_ty: Option<&TypeRef>) -> bool {
    index_ty.is_some_and(type_may_be_range)
}

fn type_may_be_int(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Int => true,
        TypeRef::Nullable(inner) => type_may_be_int(inner),
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => {
            members.iter().any(type_may_be_int)
        }
        _ => false,
    }
}

fn type_may_be_range(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Range | TypeRef::RangeInclusive => true,
        TypeRef::Nullable(inner) => type_may_be_range(inner),
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => {
            members.iter().any(type_may_be_range)
        }
        _ => false,
    }
}
