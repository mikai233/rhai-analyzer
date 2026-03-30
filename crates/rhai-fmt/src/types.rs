#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatResult {
    pub text: String,
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeFormatResult {
    pub range: rhai_syntax::TextRange,
    pub text: String,
    pub changed: bool,
}
