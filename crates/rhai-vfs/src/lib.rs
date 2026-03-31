use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod path;
pub use path::VfsPath;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FileId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DocumentVersion(pub i32);

#[derive(Debug, Clone)]
pub struct DocumentData {
    path: VfsPath,
    text: Arc<str>,
    version: DocumentVersion,
}

impl DocumentData {
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn vfs_path(&self) -> &VfsPath {
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
    ids_by_path: HashMap<VfsPath, FileId>,
}

impl VirtualFileSystem {
    pub fn set_file(
        &mut self,
        path: impl Into<VfsPath>,
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

    pub fn remove_file(&mut self, path: &Path) -> Option<FileId> {
        let path = VfsPath::new(path);
        let file_id = self.ids_by_path.remove(&path)?;
        self.files_by_id.remove(&file_id);
        Some(file_id)
    }

    pub fn file(&self, file_id: FileId) -> Option<&DocumentData> {
        self.files_by_id.get(&file_id)
    }

    pub fn file_id(&self, path: &Path) -> Option<FileId> {
        self.ids_by_path.get(&VfsPath::new(path)).copied()
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
    VfsPath::new(path).as_path().to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::{DocumentVersion, VfsPath, VirtualFileSystem, normalize_path};
    use std::path::Path;

    #[test]
    fn normalizes_paths_for_file_identity() {
        let mut vfs = VirtualFileSystem::default();
        let file_id = vfs.set_file(
            "src\\..\\src\\main.rhai",
            "let value = 1;",
            DocumentVersion(1),
        );

        // Both normalize to the same logical path.
        assert_eq!(
            normalize_path(Path::new("src/./main.rhai")),
            normalize_path(Path::new("src/main.rhai")),
        );
        // Same logical file → same FileId.
        assert_eq!(vfs.file_id(Path::new("src/main.rhai")), Some(file_id));
        let stored = vfs.file(file_id).expect("expected file");
        assert!(stored.path().ends_with("src/main.rhai"));
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

    #[test]
    fn mixed_separators_produce_same_identity() {
        let a = VfsPath::new("src\\lib\\util.rhai");
        let b = VfsPath::new("src/lib/util.rhai");
        assert_eq!(a, b);
    }

    #[test]
    fn relative_paths_remain_relative() {
        assert_eq!(
            normalize_path(Path::new("src/./main.rhai")),
            Path::new("src/main.rhai")
        );
        assert_eq!(
            normalize_path(Path::new("src/../main.rhai")),
            Path::new("main.rhai")
        );
    }

    #[test]
    fn drive_letter_is_uppercased() {
        let a = VfsPath::new_real_path("c:/Users/foo/main.rhai");
        let b = VfsPath::new_real_path("C:/Users/foo/main.rhai");
        assert_eq!(a, b);
        assert!(a.to_string().starts_with("C:"));
    }

    #[test]
    fn windows_drive_paths_are_lexically_normalized() {
        let path = VfsPath::new_real_path("c:\\Users\\foo\\..\\bar\\main.rhai");
        assert_eq!(path.to_string(), "C:/Users/bar/main.rhai");
    }

    #[test]
    fn virtual_paths_are_normalized_without_host_path_rules() {
        let path = VfsPath::new_virtual_path("fixtures\\src\\..\\main.rhai");
        assert_eq!(path.to_string(), "fixtures/main.rhai");
    }
}
