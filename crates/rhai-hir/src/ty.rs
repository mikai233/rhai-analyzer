use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeRef {
    Unknown,
    Any,
    Never,
    Dynamic,
    Bool,
    Int,
    Float,
    Decimal,
    String,
    Char,
    Blob,
    Timestamp,
    FnPtr,
    Unit,
    Range,
    RangeInclusive,
    Named(String),
    Applied { name: String, args: Vec<TypeRef> },
    Object(BTreeMap<String, TypeRef>),
    Array(Box<TypeRef>),
    Map(Box<TypeRef>, Box<TypeRef>),
    Nullable(Box<TypeRef>),
    Union(Vec<TypeRef>),
    Function(FunctionTypeRef),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionTypeRef {
    pub params: Vec<TypeRef>,
    pub ret: Box<TypeRef>,
}

pub fn parse_type_ref(text: &str) -> Option<TypeRef> {
    let mut parser = Parser::new(text);
    let ty = parser.parse_type()?;
    parser.skip_ws();
    (parser.is_eof()).then_some(ty)
}

struct Parser<'a> {
    text: &'a str,
    cursor: usize,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, cursor: 0 }
    }

    fn parse_type(&mut self) -> Option<TypeRef> {
        self.parse_union_type()
    }

    fn parse_union_type(&mut self) -> Option<TypeRef> {
        let mut members = vec![self.parse_nullable_type()?];

        loop {
            self.skip_ws();
            if !self.eat_char('|') {
                break;
            }
            members.push(self.parse_nullable_type()?);
        }

        if members.len() == 1 {
            members.pop()
        } else {
            Some(TypeRef::Union(members))
        }
    }

    fn parse_nullable_type(&mut self) -> Option<TypeRef> {
        let mut ty = self.parse_primary_type()?;

        loop {
            self.skip_ws();
            if !self.eat_char('?') {
                break;
            }
            ty = TypeRef::Nullable(Box::new(ty));
        }

        Some(ty)
    }

    fn parse_primary_type(&mut self) -> Option<TypeRef> {
        self.skip_ws();

        if self.eat_char('(') {
            self.skip_ws();
            if self.eat_char(')') {
                return Some(TypeRef::Unit);
            }
            let ty = self.parse_type()?;
            self.skip_ws();
            self.expect_char(')')?;
            return Some(ty);
        }

        let name = self.parse_ident()?;
        match name.as_str() {
            "any" => Some(TypeRef::Any),
            "unknown" => Some(TypeRef::Unknown),
            "never" => Some(TypeRef::Never),
            "dynamic" | "Dynamic" => Some(TypeRef::Dynamic),
            "bool" => Some(TypeRef::Bool),
            "int" => Some(TypeRef::Int),
            "float" => Some(TypeRef::Float),
            "decimal" => Some(TypeRef::Decimal),
            "string" => Some(TypeRef::String),
            "char" => Some(TypeRef::Char),
            "blob" => Some(TypeRef::Blob),
            "timestamp" => Some(TypeRef::Timestamp),
            "Fn" | "FnPtr" => Some(TypeRef::FnPtr),
            "range" => {
                if self.eat_char('=') {
                    Some(TypeRef::RangeInclusive)
                } else {
                    Some(TypeRef::Range)
                }
            }
            "array" => {
                let inner = self.parse_single_generic()?;
                Some(TypeRef::Array(Box::new(inner)))
            }
            "map" => {
                let (key, value) = self.parse_pair_generic()?;
                Some(TypeRef::Map(Box::new(key), Box::new(value)))
            }
            "fun" => self.parse_function_type(),
            other => {
                if self.peek_char() == Some('<') {
                    let args = self.parse_generic_args()?;
                    Some(TypeRef::Applied {
                        name: other.to_owned(),
                        args,
                    })
                } else {
                    Some(TypeRef::Named(other.to_owned()))
                }
            }
        }
    }

    fn parse_function_type(&mut self) -> Option<TypeRef> {
        self.skip_ws();
        self.expect_char('(')?;
        let mut params = Vec::new();

        loop {
            self.skip_ws();
            if self.eat_char(')') {
                break;
            }

            params.push(self.parse_type()?);
            self.skip_ws();

            if self.eat_char(',') {
                continue;
            }

            self.expect_char(')')?;
            break;
        }

        self.skip_ws();
        self.expect_str("->")?;
        let ret = self.parse_type()?;

        Some(TypeRef::Function(FunctionTypeRef {
            params,
            ret: Box::new(ret),
        }))
    }

    fn parse_single_generic(&mut self) -> Option<TypeRef> {
        self.skip_ws();
        self.expect_char('<')?;
        let inner = self.parse_type()?;
        self.skip_ws();
        self.expect_char('>')?;
        Some(inner)
    }

    fn parse_pair_generic(&mut self) -> Option<(TypeRef, TypeRef)> {
        self.skip_ws();
        self.expect_char('<')?;
        let first = self.parse_type()?;
        self.skip_ws();
        self.expect_char(',')?;
        let second = self.parse_type()?;
        self.skip_ws();
        self.expect_char('>')?;
        Some((first, second))
    }

    fn parse_generic_args(&mut self) -> Option<Vec<TypeRef>> {
        self.skip_ws();
        self.expect_char('<')?;
        let mut args = Vec::new();

        loop {
            args.push(self.parse_type()?);
            self.skip_ws();

            if self.eat_char(',') {
                continue;
            }

            self.expect_char('>')?;
            break;
        }

        Some(args)
    }

    fn parse_ident(&mut self) -> Option<String> {
        self.skip_ws();
        let start = self.cursor;

        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':') {
                self.cursor += ch.len_utf8();
            } else {
                break;
            }
        }

        (self.cursor > start).then(|| self.text[start..self.cursor].to_owned())
    }

    fn expect_char(&mut self, expected: char) -> Option<()> {
        self.skip_ws();
        self.eat_char(expected).then_some(())
    }

    fn expect_str(&mut self, expected: &str) -> Option<()> {
        self.skip_ws();
        self.text[self.cursor..].starts_with(expected).then(|| {
            self.cursor += expected.len();
        })
    }

    fn eat_char(&mut self, expected: char) -> bool {
        match self.peek_char() {
            Some(ch) if ch == expected => {
                self.cursor += ch.len_utf8();
                true
            }
            _ => false,
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.text[self.cursor..].chars().next()
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.cursor += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn is_eof(&self) -> bool {
        self.cursor >= self.text.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{FunctionTypeRef, TypeRef, parse_type_ref};

    #[test]
    fn parses_basic_type_refs() {
        assert_eq!(parse_type_ref("int"), Some(TypeRef::Int));
        assert_eq!(
            parse_type_ref("string?"),
            Some(TypeRef::Nullable(Box::new(TypeRef::String)))
        );
        assert_eq!(
            parse_type_ref("array<int | string>"),
            Some(TypeRef::Array(Box::new(TypeRef::Union(vec![
                TypeRef::Int,
                TypeRef::String,
            ]))))
        );
    }

    #[test]
    fn parses_function_and_map_type_refs() {
        assert_eq!(
            parse_type_ref("fun(int, string) -> bool"),
            Some(TypeRef::Function(FunctionTypeRef {
                params: vec![TypeRef::Int, TypeRef::String],
                ret: Box::new(TypeRef::Bool),
            }))
        );
        assert_eq!(
            parse_type_ref("map<string, array<float>>"),
            Some(TypeRef::Map(
                Box::new(TypeRef::String),
                Box::new(TypeRef::Array(Box::new(TypeRef::Float))),
            ))
        );
    }

    #[test]
    fn parses_applied_named_type_refs() {
        assert_eq!(
            parse_type_ref("result<int, string?>"),
            Some(TypeRef::Applied {
                name: "result".to_owned(),
                args: vec![TypeRef::Int, TypeRef::Nullable(Box::new(TypeRef::String)),],
            })
        );
    }

    #[test]
    fn parses_core_rhai_builtin_type_refs() {
        assert_eq!(parse_type_ref("blob"), Some(TypeRef::Blob));
        assert_eq!(parse_type_ref("timestamp"), Some(TypeRef::Timestamp));
        assert_eq!(parse_type_ref("Fn"), Some(TypeRef::FnPtr));
        assert_eq!(parse_type_ref("Dynamic"), Some(TypeRef::Dynamic));
        assert_eq!(parse_type_ref("()"), Some(TypeRef::Unit));
        assert_eq!(parse_type_ref("range"), Some(TypeRef::Range));
        assert_eq!(parse_type_ref("range="), Some(TypeRef::RangeInclusive));
    }
}
