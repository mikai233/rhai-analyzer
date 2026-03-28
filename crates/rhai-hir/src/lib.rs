use rhai_syntax::{Parse, TextRange};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Variable,
    Function,
    Module,
    Type,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub range: TextRange,
}

#[derive(Debug, Clone)]
pub struct LoweredFile {
    pub root_range: TextRange,
    pub symbols: Vec<Symbol>,
}

pub fn lower_file(parse: &Parse) -> LoweredFile {
    LoweredFile {
        root_range: parse.root().range(),
        symbols: Vec::new(),
    }
}
