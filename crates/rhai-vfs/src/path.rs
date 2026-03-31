use std::fmt;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct VfsPath(String);

impl VfsPath {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();

        // Resolve to absolute path first (before separator normalization),
        // so that relative paths get a stable, cwd-based identity.
        let abs_path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());

        // Normalize all separators to forward slash, then re-parse components.
        let path_str = abs_path.to_string_lossy().replace('\\', "/");
        let path = Path::new(&path_str);

        let mut components = Vec::new();
        for component in path.components() {
            match component {
                Component::Prefix(prefix) => {
                    components.push(prefix.as_os_str().to_string_lossy().to_string())
                }
                Component::RootDir => components.push("/".to_string()),
                Component::CurDir => {}
                Component::ParentDir => {
                    if let Some(last) = components.last() {
                        if last != "/" && last != ".." && !last.ends_with(':') {
                            components.pop();
                        } else {
                            components.push("..".to_string());
                        }
                    } else {
                        components.push("..".to_string());
                    }
                }
                Component::Normal(segment) => {
                    components.push(segment.to_string_lossy().to_string())
                }
            }
        }

        if components.is_empty() {
            if !path_str.is_empty() {
                return VfsPath(".".to_string());
            }
            return VfsPath(String::new());
        }

        let mut res = String::new();
        for (i, c) in components.iter().enumerate() {
            if i > 0 && c != "/" && !res.ends_with('/') {
                res.push('/');
            }
            res.push_str(c);
        }

        // Normalize drive letter to uppercase (e.g. "c:/" → "C:/").
        uppercase_drive_letter(&mut res);

        VfsPath(res)
    }

    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
}

/// If `s` starts with a single ASCII letter followed by `':'`, upper-case it.
fn uppercase_drive_letter(s: &mut String) {
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_lowercase() && bytes[1] == b':' {
        // SAFETY: replacing one ASCII lowercase byte with its uppercase variant
        // keeps the string valid UTF-8.
        unsafe {
            s.as_bytes_mut()[0] = bytes[0].to_ascii_uppercase();
        }
    }
}

impl fmt::Display for VfsPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for VfsPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<PathBuf> for VfsPath {
    fn from(path: PathBuf) -> Self {
        VfsPath::new(path)
    }
}

impl From<&str> for VfsPath {
    fn from(path: &str) -> Self {
        VfsPath::new(path)
    }
}

impl From<String> for VfsPath {
    fn from(path: String) -> Self {
        VfsPath::new(path)
    }
}
