#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Doc {
    Nil,
    Text(String),
    Line,
    HardLine,
    SoftLine,
    Concat(Vec<Doc>),
    Indent { levels: usize, content: Box<Doc> },
    Group(Box<Doc>),
}

impl Doc {
    pub(crate) fn nil() -> Self {
        Self::Nil
    }

    pub(crate) fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    pub(crate) fn hard_line() -> Self {
        Self::HardLine
    }

    pub(crate) fn line() -> Self {
        Self::Line
    }

    pub(crate) fn soft_line() -> Self {
        Self::SoftLine
    }

    pub(crate) fn concat(parts: Vec<Doc>) -> Self {
        Self::Concat(parts)
    }

    pub(crate) fn join(parts: Vec<Doc>, separator: Doc) -> Self {
        let mut docs = Vec::new();

        for (index, part) in parts.into_iter().enumerate() {
            if index > 0 {
                docs.push(separator.clone());
            }
            docs.push(part);
        }

        Self::Concat(docs)
    }

    pub(crate) fn indent(levels: usize, content: Doc) -> Self {
        Self::Indent {
            levels,
            content: Box::new(content),
        }
    }

    pub(crate) fn group(content: Doc) -> Self {
        Self::Group(Box::new(content))
    }
}
