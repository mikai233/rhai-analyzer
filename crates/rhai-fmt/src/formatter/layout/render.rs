use crate::FormatOptions;

use crate::formatter::layout::doc::Doc;

pub(crate) fn render_doc(doc: &Doc, _options: &FormatOptions) -> String {
    render_doc_with_indent(doc, _options, 0)
}

pub(crate) fn render_doc_with_indent(
    doc: &Doc,
    options: &FormatOptions,
    base_indent: usize,
) -> String {
    let mut out = String::new();
    let mut state = RenderState {
        out: &mut out,
        options,
        base_indent,
        current_indent: 0,
        column: 0,
        pending_indent: false,
    };
    render_into(doc, &mut state, RenderMode::Break);
    out
}

#[derive(Clone, Copy)]
enum RenderMode {
    Flat,
    Break,
}

struct RenderState<'a> {
    out: &'a mut String,
    options: &'a FormatOptions,
    base_indent: usize,
    current_indent: usize,
    column: usize,
    pending_indent: bool,
}

impl RenderState<'_> {
    fn write_text(&mut self, text: &str) {
        for ch in text.chars() {
            if self.pending_indent && ch != '\n' && ch != ' ' && ch != '\t' {
                self.write_indent();
            }

            self.out.push(ch);
            if ch == '\n' {
                self.column = 0;
                self.pending_indent = false;
            } else {
                self.column = advance_column(self.column, ch, self.options.indent_width.max(1));
            }
        }
    }

    fn write_line(&mut self) {
        self.out.push('\n');
        self.column = 0;
        self.pending_indent = true;
    }

    fn write_indent(&mut self) {
        let levels = self.base_indent + self.current_indent;
        match self.options.indent_style {
            crate::IndentStyle::Spaces => {
                let spaces = " ".repeat(levels * self.options.indent_width);
                self.out.push_str(&spaces);
                self.column += spaces.chars().count();
            }
            crate::IndentStyle::Tabs => {
                let tabs = "\t".repeat(levels);
                self.out.push_str(&tabs);
                self.column += levels * self.options.indent_width.max(1);
            }
        }
        self.pending_indent = false;
    }
}

fn render_into(doc: &Doc, state: &mut RenderState<'_>, mode: RenderMode) {
    match doc {
        Doc::Nil => {}
        Doc::Text(text) => state.write_text(text),
        Doc::Line => match mode {
            RenderMode::Flat => {}
            RenderMode::Break => state.write_line(),
        },
        Doc::HardLine => state.write_line(),
        Doc::SoftLine => match mode {
            RenderMode::Flat => state.write_text(" "),
            RenderMode::Break => state.write_line(),
        },
        Doc::Concat(parts) => {
            for part in parts {
                render_into(part, state, mode);
            }
        }
        Doc::Indent { levels, content } => {
            state.current_indent += levels;
            render_into(content, state, mode);
            state.current_indent -= levels;
        }
        Doc::Group(content) => {
            let available = state.options.max_line_length.saturating_sub(state.column);
            let next_mode = if fits(
                content,
                state.column,
                available,
                state.options.indent_width.max(1),
            ) {
                RenderMode::Flat
            } else {
                RenderMode::Break
            };
            render_into(content, state, next_mode);
        }
    }
}

fn fits(doc: &Doc, start_column: usize, remaining: usize, tab_width: usize) -> bool {
    match flat_width(doc, start_column, remaining, tab_width) {
        Some(width) => width <= remaining,
        None => false,
    }
}

fn flat_width(doc: &Doc, column: usize, max_remaining: usize, tab_width: usize) -> Option<usize> {
    match doc {
        Doc::Nil => Some(0),
        Doc::Text(text) => {
            if text.contains('\n') {
                None
            } else {
                let width = text_width(text, column, tab_width);
                (width <= max_remaining).then_some(width)
            }
        }
        Doc::Line => Some(0),
        Doc::HardLine => None,
        Doc::SoftLine => Some(1),
        Doc::Concat(parts) => {
            let mut total = 0usize;
            for part in parts {
                let part_width = flat_width(
                    part,
                    column + total,
                    max_remaining.saturating_sub(total),
                    tab_width,
                )?;
                total += part_width;
            }
            Some(total)
        }
        Doc::Indent { content, .. } => flat_width(content, column, max_remaining, tab_width),
        Doc::Group(content) => flat_width(content, column, max_remaining, tab_width),
    }
}

fn text_width(text: &str, mut column: usize, tab_width: usize) -> usize {
    let start = column;
    for ch in text.chars() {
        column = advance_column(column, ch, tab_width);
    }
    column - start
}

fn advance_column(column: usize, ch: char, tab_width: usize) -> usize {
    match ch {
        '\t' => {
            let advance = tab_width - (column % tab_width);
            column + advance.max(1)
        }
        _ => column + 1,
    }
}
