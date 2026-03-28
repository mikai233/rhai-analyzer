use rhai_db::{AnalyzerDatabase, ChangeSet, DatabaseSnapshot};
use rhai_hir::{Symbol, lower_file};
use rhai_syntax::{TextRange, parse_text};
use rhai_vfs::FileId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FilePosition {
    pub file_id: FileId,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverResult {
    pub signature: String,
    pub docs: Option<String>,
}

#[derive(Debug, Default)]
pub struct AnalysisHost {
    db: AnalyzerDatabase,
}

impl AnalysisHost {
    pub fn apply_change(&mut self, change_set: ChangeSet) {
        self.db.apply_change(change_set);
    }

    pub fn snapshot(&self) -> Analysis {
        Analysis {
            db: self.db.snapshot(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Analysis {
    db: DatabaseSnapshot,
}

impl Analysis {
    pub fn diagnostics(&self, file_id: FileId) -> Vec<Diagnostic> {
        let Some(text) = self.db.file_text(file_id) else {
            return Vec::new();
        };

        let parse = parse_text(&text);
        parse
            .errors()
            .iter()
            .map(|error| Diagnostic {
                message: error.message().to_owned(),
                range: error.range(),
            })
            .collect()
    }

    pub fn hover(&self, _position: FilePosition) -> Option<HoverResult> {
        None
    }

    pub fn symbols(&self, file_id: FileId) -> Vec<Symbol> {
        let Some(text) = self.db.file_text(file_id) else {
            return Vec::new();
        };

        let parse = parse_text(&text);
        lower_file(&parse).symbols
    }
}
