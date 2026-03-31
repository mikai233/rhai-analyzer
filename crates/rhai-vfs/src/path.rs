use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct VfsPath(String);

impl VfsPath {
    pub fn new(path: impl AsRef<Path>) -> Self {
        VfsPath(normalize_lexical_path(&path.as_ref().to_string_lossy()))
    }

    /// Creates a normalized path for a real filesystem path.
    ///
    /// Unlike `std::path`, this accepts both Unix roots (`/tmp/file.rhai`) and
    /// Windows drive roots (`C:/tmp/file.rhai`) independent of the host OS.
    pub fn new_real_path(path: impl AsRef<Path>) -> Self {
        let normalized = normalize_lexical_path(&path.as_ref().to_string_lossy());
        assert!(
            is_rooted_path(&normalized),
            "expected an absolute filesystem path, got `{normalized}`"
        );
        VfsPath(normalized)
    }

    /// Creates a normalized virtual path for tests or in-memory files.
    pub fn new_virtual_path(path: impl AsRef<str>) -> Self {
        VfsPath(normalize_lexical_path(path.as_ref()))
    }

    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathRoot {
    Relative,
    Unix,
    WindowsDrive(char),
}

fn normalize_lexical_path(path: &str) -> String {
    let path = path.replace('\\', "/");
    let (root, remainder) = parse_root(&path);
    let rooted = root != PathRoot::Relative;

    let mut components = Vec::<&str>::new();
    for segment in remainder.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if components.last().is_some_and(|last| *last != "..") {
                    components.pop();
                } else if !rooted {
                    components.push("..");
                }
            }
            _ => components.push(segment),
        }
    }

    let mut normalized = match root {
        PathRoot::Relative => String::new(),
        PathRoot::Unix => "/".to_string(),
        PathRoot::WindowsDrive(letter) => format!("{}:/", letter.to_ascii_uppercase()),
    };

    if !components.is_empty() {
        if !normalized.is_empty() && !normalized.ends_with('/') {
            normalized.push('/');
        }
        normalized.push_str(&components.join("/"));
    }

    if normalized.is_empty() && !path.is_empty() {
        ".".to_string()
    } else {
        normalized
    }
}

fn parse_root(path: &str) -> (PathRoot, &str) {
    let bytes = path.as_bytes();
    if path.starts_with('/') {
        return (PathRoot::Unix, path.trim_start_matches('/'));
    }

    if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/' {
        return (PathRoot::WindowsDrive(bytes[0] as char), &path[3..]);
    }

    (PathRoot::Relative, path)
}

fn is_rooted_path(s: &str) -> bool {
    let bytes = s.as_bytes();
    s.starts_with('/')
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && bytes[2] == b'/')
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
