use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FileId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DocumentVersion(pub i32);

#[derive(Debug, Clone)]
pub struct DocumentData {
    path: PathBuf,
    text: Arc<str>,
    version: DocumentVersion,
}

impl DocumentData {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn text_arc(&self) -> Arc<str> {
        Arc::clone(&self.text)
    }

    pub fn version(&self) -> DocumentVersion {
        self.version
    }
}

#[derive(Debug, Clone, Default)]
pub struct VirtualFileSystem {
    next_file_id: u32,
    files_by_id: HashMap<FileId, DocumentData>,
    ids_by_path: HashMap<PathBuf, FileId>,
}

impl VirtualFileSystem {
    pub fn set_file(
        &mut self,
        path: impl Into<PathBuf>,
        text: impl Into<String>,
        version: DocumentVersion,
    ) -> FileId {
        let path = path.into();

        if let Some(file_id) = self.ids_by_path.get(&path).copied() {
            self.files_by_id.insert(
                file_id,
                DocumentData {
                    path,
                    text: Arc::<str>::from(text.into()),
                    version,
                },
            );
            return file_id;
        }

        let file_id = FileId(self.next_file_id);
        self.next_file_id += 1;
        self.ids_by_path.insert(path.clone(), file_id);
        self.files_by_id.insert(
            file_id,
            DocumentData {
                path,
                text: Arc::<str>::from(text.into()),
                version,
            },
        );
        file_id
    }

    pub fn file(&self, file_id: FileId) -> Option<&DocumentData> {
        self.files_by_id.get(&file_id)
    }

    pub fn file_id(&self, path: &Path) -> Option<FileId> {
        self.ids_by_path.get(path).copied()
    }

    pub fn file_text(&self, file_id: FileId) -> Option<Arc<str>> {
        self.file(file_id).map(DocumentData::text_arc)
    }

    pub fn iter(&self) -> impl Iterator<Item = (FileId, &DocumentData)> {
        self.files_by_id
            .iter()
            .map(|(&file_id, data)| (file_id, data))
    }
}
