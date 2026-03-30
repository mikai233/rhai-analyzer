use std::path::PathBuf;

use rhai_syntax::{TextRange, TextSize};
use rhai_vfs::FileId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub range: TextRange,
    pub new_text: String,
}

impl TextEdit {
    pub fn replace(range: TextRange, new_text: impl Into<String>) -> Self {
        Self {
            range,
            new_text: new_text.into(),
        }
    }

    pub fn insert(offset: TextSize, new_text: impl Into<String>) -> Self {
        Self::replace(TextRange::new(offset, offset), new_text)
    }

    pub fn insertion_offset(&self) -> Option<u32> {
        (self.range.start() == self.range.end()).then(|| u32::from(self.range.start()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTextEdit {
    pub file_id: FileId,
    pub edits: Vec<TextEdit>,
}

impl FileTextEdit {
    pub fn new(file_id: FileId, edits: Vec<TextEdit>) -> Self {
        Self { file_id, edits }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRename {
    pub file_id: FileId,
    pub new_path: PathBuf,
}

impl FileRename {
    pub fn new(file_id: FileId, new_path: PathBuf) -> Self {
        Self { file_id, new_path }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceChange {
    pub file_edits: Vec<FileTextEdit>,
    pub file_renames: Vec<FileRename>,
}

impl SourceChange {
    pub fn new(file_edits: Vec<FileTextEdit>) -> Self {
        Self {
            file_edits,
            file_renames: Vec::new(),
        }
    }

    pub fn with_file_renames(mut self, file_renames: Vec<FileRename>) -> Self {
        self.file_renames = file_renames;
        self
    }

    pub fn from_text_edit(file_id: FileId, edit: TextEdit) -> Self {
        Self::new(vec![FileTextEdit::new(file_id, vec![edit])])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoImportAction {
    pub label: String,
    pub module_name: String,
    pub provider_file_id: FileId,
    pub source_change: SourceChange,
}
