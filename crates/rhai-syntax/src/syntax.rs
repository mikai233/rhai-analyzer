pub use text_size::{TextRange, TextSize};

use std::sync::Arc;

use thiserror::Error;

use crate::TriviaStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Whitespace,          // spaces, tabs, newlines
    LineComment,         // `// ...`
    DocLineComment,      // `/// ...` or `//! ...`
    BlockComment,        // `/* ... */`
    DocBlockComment,     // `/** ... */` or `/*! ... */`
    Shebang,             // `#!...` at the start of the file
    Ident,               // identifier
    Underscore,          // `_`
    Int,                 // integer literal
    Float,               // float literal
    String,              // `"..."` string literal
    RawString,           // raw string literal
    BacktickString,      // `` `...` `` string literal
    Backtick,            // `` ` `` inside interpolated string structure
    StringText,          // plain text segment inside interpolated string
    InterpolationStart,  // `${`
    Char,                // `'x'` character literal
    LetKw,               // `let`
    ConstKw,             // `const`
    IfKw,                // `if`
    ElseKw,              // `else`
    SwitchKw,            // `switch`
    DoKw,                // `do`
    WhileKw,             // `while`
    UntilKw,             // `until`
    LoopKw,              // `loop`
    ForKw,               // `for`
    InKw,                // `in`
    ContinueKw,          // `continue`
    BreakKw,             // `break`
    ReturnKw,            // `return`
    ThrowKw,             // `throw`
    TryKw,               // `try`
    CatchKw,             // `catch`
    ImportKw,            // `import`
    ExportKw,            // `export`
    AsKw,                // `as`
    GlobalKw,            // `global`
    PrivateKw,           // `private`
    FnKw,                // `fn`
    ThisKw,              // `this`
    TrueKw,              // `true`
    FalseKw,             // `false`
    FnPtrKw,             // `Fn`
    CallKw,              // `call`
    CurryKw,             // `curry`
    IsSharedKw,          // `is_shared`
    IsDefFnKw,           // `is_def_fn`
    IsDefVarKw,          // `is_def_var`
    TypeOfKw,            // `type_of`
    PrintKw,             // `print`
    DebugKw,             // `debug`
    EvalKw,              // `eval`
    OpenParen,           // `(`
    CloseParen,          // `)`
    OpenBrace,           // `{`
    CloseBrace,          // `}`
    OpenBracket,         // `[`
    CloseBracket,        // `]`
    HashBraceOpen,       // `#{`
    Comma,               // `,`
    Semicolon,           // `;`
    Colon,               // `:`
    ColonColon,          // `::`
    Dot,                 // `.`
    QuestionDot,         // `?.`
    QuestionOpenBracket, // `?[`
    FatArrow,            // `=>`
    Eq,                  // `=`
    PlusEq,              // `+=`
    MinusEq,             // `-=`
    StarEq,              // `*=`
    SlashEq,             // `/=`
    PercentEq,           // `%=`
    StarStarEq,          // `**=`
    ShlEq,               // `<<=`
    ShrEq,               // `>>=`
    AmpEq,               // `&=`
    PipeEq,              // `|=`
    CaretEq,             // `^=`
    QuestionQuestionEq,  // `??=`
    Plus,                // `+`
    Minus,               // `-`
    Star,                // `*`
    Slash,               // `/`
    Percent,             // `%`
    StarStar,            // `**`
    Shl,                 // `<<`
    Shr,                 // `>>`
    Amp,                 // `&`
    Pipe,                // `|`
    Caret,               // `^`
    AmpAmp,              // `&&`
    PipePipe,            // `||`
    Bang,                // `!`
    EqEq,                // `==`
    BangEq,              // `!=`
    Gt,                  // `>`
    GtEq,                // `>=`
    Lt,                  // `<`
    LtEq,                // `<=`
    QuestionQuestion,    // `??`
    Range,               // `..`
    RangeEq,             // `..=`
    Unknown,             // unrecognized token
}

impl TokenKind {
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace
                | Self::LineComment
                | Self::DocLineComment
                | Self::BlockComment
                | Self::DocBlockComment
                | Self::Shebang
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyntaxToken {
    kind: TokenKind,
    range: TextRange,
}

impl SyntaxToken {
    pub fn new(kind: TokenKind, range: TextRange) -> Self {
        Self { kind, range }
    }

    pub fn kind(self) -> TokenKind {
        self.kind
    }

    pub fn range(self) -> TextRange {
        self.range
    }

    pub fn text(self, source: &str) -> &str {
        let start = u32::from(self.range.start()) as usize;
        let end = u32::from(self.range.end()) as usize;
        &source[start..end]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxKind {
    Root,                   // whole file / parse root
    ItemFn,                 // function item
    StmtLet,                // `let` statement
    StmtConst,              // `const` statement
    StmtImport,             // `import` statement
    StmtExport,             // `export` statement
    StmtBreak,              // `break` statement
    StmtContinue,           // `continue` statement
    StmtReturn,             // `return` statement
    StmtThrow,              // `throw` statement
    StmtTry,                // `try` statement
    StmtExpr,               // expression statement
    ExprName,               // name / identifier expression
    ExprLiteral,            // literal expression
    ExprArray,              // array literal
    ExprObject,             // object map literal
    ExprIf,                 // `if` expression
    ExprSwitch,             // `switch` expression
    ExprWhile,              // `while` expression
    ExprLoop,               // `loop` expression
    ExprFor,                // `for` expression
    ExprDo,                 // `do ... while/until` expression
    ExprPath,               // `a::b::c` path expression
    ExprClosure,            // closure expression
    ExprInterpolatedString, // interpolated back-tick string
    ExprUnary,              // unary operator expression
    ExprBinary,             // binary operator expression
    ExprAssign,             // assignment / compound assignment expression
    ExprParen,              // parenthesized expression
    ExprCall,               // function or method call expression
    ExprIndex,              // indexing / safe indexing expression
    ExprField,              // field / property access expression
    RootItemList,           // root-level item list
    BlockItemList,          // block item list
    ArgList,                // call argument list
    ParamList,              // function parameter list
    ClosureParamList,       // closure parameter list
    ArrayItemList,          // array element list
    ObjectFieldList,        // object field list
    ObjectField,            // object field entry
    SwitchArmList,          // switch arm list
    StringPartList,         // parts inside an interpolated string
    StringSegment,          // plain text segment inside interpolation
    StringInterpolation,    // `${ ... }` interpolation segment
    InterpolationBody,      // parsed body inside `${ ... }`
    InterpolationItemList,  // item list inside `${ ... }`
    ElseBranch,             // `else` branch
    ForBindings,            // binding list in `for`
    AliasClause,            // `as alias`
    SwitchArm,              // one `switch` arm
    SwitchPatternList,      // pattern list before `=>`
    DoCondition,            // trailing `while/until` condition
    CatchClause,            // `catch` clause
    Block,                  // `{ ... }` block
    Error,                  // recovery / missing node
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxElement {
    Node(Box<SyntaxNode>),
    Token(SyntaxToken),
}

impl SyntaxElement {
    pub fn as_node(&self) -> Option<&SyntaxNode> {
        match self {
            Self::Node(node) => Some(node),
            Self::Token(_) => None,
        }
    }

    pub fn as_token(&self) -> Option<SyntaxToken> {
        match self {
            Self::Node(_) => None,
            Self::Token(token) => Some(*token),
        }
    }

    pub fn range(&self) -> TextRange {
        match self {
            Self::Node(node) => node.range(),
            Self::Token(token) => token.range(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxNode {
    kind: SyntaxKind,
    range: TextRange,
    children: Vec<SyntaxElement>,
}

impl SyntaxNode {
    pub fn new(kind: SyntaxKind, children: Vec<SyntaxElement>, fallback_offset: TextSize) -> Self {
        let range = match (children.first(), children.last()) {
            (Some(first), Some(last)) => TextRange::new(first.range().start(), last.range().end()),
            _ => TextRange::new(fallback_offset, fallback_offset),
        };

        Self {
            kind,
            range,
            children,
        }
    }

    pub fn with_range(kind: SyntaxKind, range: TextRange, children: Vec<SyntaxElement>) -> Self {
        Self {
            kind,
            range,
            children,
        }
    }

    pub fn kind(&self) -> SyntaxKind {
        self.kind
    }

    pub fn range(&self) -> TextRange {
        self.range
    }

    pub fn children(&self) -> &[SyntaxElement] {
        self.raw_children()
    }

    pub fn raw_children(&self) -> &[SyntaxElement] {
        &self.children
    }

    pub fn significant_children(&self) -> impl Iterator<Item = &SyntaxElement> {
        self.raw_children().iter().filter(|element| match element {
            SyntaxElement::Token(token) => !token.kind().is_trivia(),
            SyntaxElement::Node(_) => true,
        })
    }

    pub fn child_nodes(&self) -> impl Iterator<Item = &SyntaxNode> {
        self.raw_children()
            .iter()
            .filter_map(SyntaxElement::as_node)
    }

    pub fn raw_tokens(&self) -> impl Iterator<Item = SyntaxToken> + '_ {
        self.raw_children()
            .iter()
            .filter_map(SyntaxElement::as_token)
    }

    pub fn significant_tokens(&self) -> impl Iterator<Item = SyntaxToken> + '_ {
        self.raw_tokens().filter(|token| !token.kind().is_trivia())
    }

    pub fn first_significant_token(&self) -> Option<SyntaxToken> {
        first_significant_token_in(self)
    }

    pub fn last_significant_token(&self) -> Option<SyntaxToken> {
        last_significant_token_in(self)
    }

    pub fn significant_range(&self) -> TextRange {
        match (
            self.first_significant_token(),
            self.last_significant_token(),
        ) {
            (Some(first), Some(last)) => TextRange::new(first.range().start(), last.range().end()),
            _ => self.range(),
        }
    }

    pub fn structural_range(&self) -> TextRange {
        let end = self
            .last_significant_token()
            .map(|token| token.range().end())
            .unwrap_or_else(|| self.range().end());
        TextRange::new(self.range().start(), end.max(self.range().start()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct SyntaxError {
    message: String,
    range: TextRange,
}

impl SyntaxError {
    pub fn new(message: impl Into<String>, range: TextRange) -> Self {
        Self {
            message: message.into(),
            range,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn range(&self) -> TextRange {
        self.range
    }
}

#[derive(Debug, Clone)]
pub struct Parse {
    text: Arc<str>,
    tokens: Vec<SyntaxToken>,
    trivia: TriviaStore,
    root: SyntaxNode,
    errors: Vec<SyntaxError>,
}

impl Parse {
    pub fn new(
        text: Arc<str>,
        tokens: Vec<SyntaxToken>,
        root: SyntaxNode,
        errors: Vec<SyntaxError>,
    ) -> Self {
        let trivia = TriviaStore::new(&text, &tokens);
        Self {
            text,
            tokens,
            trivia,
            root,
            errors,
        }
    }

    pub fn root(&self) -> &SyntaxNode {
        &self.root
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn tokens(&self) -> &[SyntaxToken] {
        &self.tokens
    }

    pub fn trivia(&self) -> &TriviaStore {
        &self.trivia
    }

    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }

    pub fn debug_tree(&self) -> String {
        let mut out = String::new();
        self.write_node(&mut out, &self.root, 0);
        out
    }

    pub fn debug_tree_compact(&self) -> String {
        let mut out = String::new();
        self.write_compact_node(&mut out, &self.root, 0);
        out
    }

    fn write_node(&self, out: &mut String, node: &SyntaxNode, indent: usize) {
        push_indent(out, indent);
        push_range(out, node.range());
        out.push_str(&format!("{:?}\n", node.kind()));

        for child in node.children() {
            match child {
                SyntaxElement::Node(node) => self.write_node(out, node, indent + 2),
                SyntaxElement::Token(token) => {
                    if token.kind().is_trivia() {
                        continue;
                    }
                    push_indent(out, indent + 2);
                    push_range(out, token.range());
                    out.push_str(&format!(
                        "{:?} {:?}\n",
                        token.kind(),
                        token.text(&self.text)
                    ));
                }
            }
        }
    }

    fn write_compact_node(&self, out: &mut String, node: &SyntaxNode, indent: usize) {
        push_indent(out, indent);
        out.push_str(&format!("{:?}\n", node.kind()));

        for child in node.children() {
            match child {
                SyntaxElement::Node(node) => self.write_compact_node(out, node, indent + 2),
                SyntaxElement::Token(token) => {
                    if token.kind().is_trivia() {
                        continue;
                    }
                    push_indent(out, indent + 2);
                    out.push_str(&format!(
                        "{:?} {:?}\n",
                        token.kind(),
                        token.text(&self.text)
                    ));
                }
            }
        }
    }
}

pub(crate) fn node_element(node: SyntaxNode) -> SyntaxElement {
    SyntaxElement::Node(Box::new(node))
}

pub(crate) fn token_element(token: SyntaxToken) -> SyntaxElement {
    SyntaxElement::Token(token)
}

pub(crate) fn empty_range(offset: TextSize) -> TextRange {
    TextRange::new(offset, offset)
}

pub(crate) fn text_size_of(text: &str) -> TextSize {
    TextSize::from(u32::try_from(text.len()).unwrap_or(u32::MAX))
}

fn push_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push(' ');
    }
}

fn push_range(out: &mut String, range: TextRange) {
    let start = u32::from(range.start());
    let end = u32::from(range.end());
    out.push_str(&format!("{start}..{end} "));
}

fn first_significant_token_in(node: &SyntaxNode) -> Option<SyntaxToken> {
    for child in node.raw_children() {
        match child {
            SyntaxElement::Token(token) if !token.kind().is_trivia() => return Some(*token),
            SyntaxElement::Node(child_node) => {
                if let Some(token) = first_significant_token_in(child_node) {
                    return Some(token);
                }
            }
            _ => {}
        }
    }

    None
}

fn last_significant_token_in(node: &SyntaxNode) -> Option<SyntaxToken> {
    for child in node.raw_children().iter().rev() {
        match child {
            SyntaxElement::Token(token) if !token.kind().is_trivia() => return Some(*token),
            SyntaxElement::Node(child_node) => {
                if let Some(token) = last_significant_token_in(child_node) {
                    return Some(token);
                }
            }
            _ => {}
        }
    }

    None
}
