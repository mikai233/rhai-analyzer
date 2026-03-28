use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
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
        let path = normalize_path(&path.into());

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

    pub fn remove_file(&mut self, path: &Path) -> Option<FileId> {
        let path = normalize_path(path);
        let file_id = self.ids_by_path.remove(&path)?;
        self.files_by_id.remove(&file_id);
        Some(file_id)
    }

    pub fn file(&self, file_id: FileId) -> Option<&DocumentData> {
        self.files_by_id.get(&file_id)
    }

    pub fn file_id(&self, path: &Path) -> Option<FileId> {
        self.ids_by_path.get(&normalize_path(path)).copied()
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

pub fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }

    if normalized.as_os_str().is_empty() && !path.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::{DocumentVersion, VirtualFileSystem, normalize_path};
    use std::path::Path;

    #[test]
    fn normalizes_paths_for_file_identity() {
        let mut vfs = VirtualFileSystem::default();
        let file_id = vfs.set_file(
            "src\\..\\src\\main.rhai",
            "let value = 1;",
            DocumentVersion(1),
        );

        assert_eq!(
            normalize_path(Path::new("src/./main.rhai")),
            Path::new("src/main.rhai")
        );
        assert_eq!(vfs.file_id(Path::new("src/main.rhai")), Some(file_id));
        assert_eq!(
            vfs.file(file_id).expect("expected file").path(),
            Path::new("src/main.rhai")
        );
    }

    #[test]
    fn removes_files_by_normalized_path() {
        let mut vfs = VirtualFileSystem::default();
        let file_id = vfs.set_file("workspace/one.rhai", "let value = 1;", DocumentVersion(1));

        assert_eq!(
            vfs.remove_file(Path::new("workspace/./one.rhai")),
            Some(file_id)
        );
        assert!(vfs.file(file_id).is_none());
        assert!(vfs.file_id(Path::new("workspace/one.rhai")).is_none());
    }
}
