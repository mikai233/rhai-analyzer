use std::path::PathBuf;
use std::sync::Arc;

use rhai_project::ProjectConfig;
use rhai_vfs::{DocumentVersion, FileId, VirtualFileSystem};

#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub text: String,
    pub version: DocumentVersion,
}

#[derive(Debug, Clone, Default)]
pub struct ChangeSet {
    pub files: Vec<FileChange>,
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
            project: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseSnapshot {
    vfs: Arc<VirtualFileSystem>,
    project: Arc<ProjectConfig>,
}

impl DatabaseSnapshot {
    pub fn vfs(&self) -> &VirtualFileSystem {
        &self.vfs
    }

    pub fn project(&self) -> &ProjectConfig {
        &self.project
    }

    pub fn file_text(&self, file_id: FileId) -> Option<Arc<str>> {
        self.vfs.file_text(file_id)
    }
}

#[derive(Debug, Default)]
pub struct AnalyzerDatabase {
    vfs: VirtualFileSystem,
    project: ProjectConfig,
}

impl AnalyzerDatabase {
    pub fn apply_change(&mut self, change_set: ChangeSet) {
        for change in change_set.files {
            self.vfs.set_file(change.path, change.text, change.version);
        }

        if let Some(project) = change_set.project {
            self.project = project;
        }
    }

    pub fn snapshot(&self) -> DatabaseSnapshot {
        DatabaseSnapshot {
            vfs: Arc::new(self.vfs.clone()),
            project: Arc::new(self.project.clone()),
        }
    }
}
