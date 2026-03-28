use std::path::PathBuf;

use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;

#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub text: String,
    pub version: DocumentVersion,
}

#[derive(Debug, Clone, Default)]
pub struct ChangeSet {
    pub files: Vec<FileChange>,
    pub removed_files: Vec<PathBuf>,
    pub project: Option<ProjectConfig>,
}

impl ChangeSet {
    pub fn single_file(
        path: impl Into<PathBuf>,
        text: impl Into<String>,
        version: DocumentVersion,
    ) -> Self {
        Self {
            files: vec![FileChange {
                path: path.into(),
                text: text.into(),
                version,
            }],
            removed_files: Vec::new(),
            project: None,
        }
    }

    pub fn remove_file(path: impl Into<PathBuf>) -> Self {
        Self {
            files: Vec::new(),
            removed_files: vec![path.into()],
            project: None,
        }
    }
}
