#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndentStyle {
    Spaces,
    Tabs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormatMode {
    Document,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContainerLayoutStyle {
    Auto,
    PreferSingleLine,
    PreferMultiLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportSortOrder {
    Preserve,
    ModulePath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatOptions {
    pub mode: FormatMode,
    pub indent_style: IndentStyle,
    pub indent_width: usize,
    pub max_line_length: usize,
    pub trailing_commas: bool,
    pub final_newline: bool,
    pub container_layout: ContainerLayoutStyle,
    pub import_sort_order: ImportSortOrder,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            mode: FormatMode::Document,
            indent_style: IndentStyle::Spaces,
            indent_width: 4,
            max_line_length: 100,
            trailing_commas: true,
            final_newline: true,
            container_layout: ContainerLayoutStyle::Auto,
            import_sort_order: ImportSortOrder::Preserve,
        }
    }
}
