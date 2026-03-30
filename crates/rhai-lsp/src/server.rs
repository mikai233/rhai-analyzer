use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use lsp_types::Uri;
use rhai_db::{ChangeImpact, ChangeSet};
use rhai_ide::AnalysisHost;
use rhai_vfs::{DocumentVersion, FileId, normalize_path};

const DEFAULT_QUERY_SUPPORT_BUDGET: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedDocument {
    pub uri: Uri,
    pub normalized_path: PathBuf,
    pub version: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticUpdate {
    pub uri: Uri,
    pub version: Option<i32>,
    pub diagnostics: Vec<rhai_ide::Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeActionEdit {
    pub title: String,
    pub uri: Uri,
    pub version: Option<i32>,
    pub insert_offset: u32,
    pub insert_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSymbolMatch {
    pub uri: Uri,
    pub symbol: rhai_ide::WorkspaceSymbol,
}

#[derive(Debug)]
pub struct Server {
    pub(crate) analysis_host: AnalysisHost,
    pub(crate) open_documents: HashMap<Uri, ManagedDocument>,
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

impl Server {
    pub fn new() -> Self {
        let mut analysis_host = AnalysisHost::default();
        let _ = analysis_host.set_query_support_budget(Some(DEFAULT_QUERY_SUPPORT_BUDGET));

        Self {
            analysis_host,
            open_documents: HashMap::new(),
        }
    }

    pub fn analysis_host(&self) -> &AnalysisHost {
        &self.analysis_host
    }

    pub fn open_documents(&self) -> Vec<ManagedDocument> {
        let mut documents = self.open_documents.values().cloned().collect::<Vec<_>>();
        documents.sort_by(|left, right| left.normalized_path.cmp(&right.normalized_path));
        documents
    }

    pub fn open_document(
        &mut self,
        uri: Uri,
        version: i32,
        text: impl Into<String>,
    ) -> Result<Vec<DiagnosticUpdate>> {
        self.upsert_document(uri, version, text)
    }

    pub fn change_document(
        &mut self,
        uri: Uri,
        version: i32,
        text: impl Into<String>,
    ) -> Result<Vec<DiagnosticUpdate>> {
        self.upsert_document(uri, version, text)
    }

    pub(crate) fn upsert_document(
        &mut self,
        uri: Uri,
        version: i32,
        text: impl Into<String>,
    ) -> Result<Vec<DiagnosticUpdate>> {
        let normalized_path = path_from_uri(&uri)?;
        let managed_document = ManagedDocument {
            uri: uri.clone(),
            normalized_path: normalized_path.clone(),
            version,
        };

        let impact = self
            .analysis_host
            .apply_change_report(ChangeSet::single_file(
                normalized_path.clone(),
                text,
                DocumentVersion(version),
            ));
        self.open_documents.insert(uri, managed_document);
        self.warm_hot_files(&impact);
        Ok(self.diagnostic_updates_for_impact(&impact))
    }

    pub(crate) fn warm_hot_files(&mut self, impact: &ChangeImpact) {
        let snapshot = self.analysis_host.snapshot();
        let mut hot_files = BTreeSet::<FileId>::new();

        for document in self.open_documents.values() {
            let Some(file_id) = snapshot.file_id_for_path(&document.normalized_path) else {
                continue;
            };

            if impact.rebuilt_files.contains(&file_id)
                || impact.dependency_affected_files.contains(&file_id)
            {
                hot_files.insert(file_id);
            }
        }

        if !hot_files.is_empty() {
            self.analysis_host
                .warm_query_support(&hot_files.into_iter().collect::<Vec<_>>());
        }
    }
}

pub(crate) fn path_from_uri(uri: &Uri) -> Result<PathBuf> {
    let raw = uri.as_str();
    let encoded_path = raw
        .strip_prefix("file:///")
        .ok_or_else(|| anyhow!("expected a file:// URI, got `{raw}`"))?;
    let decoded_path = encoded_path.replace("%20", " ");

    #[cfg(windows)]
    let path = PathBuf::from(decoded_path.replace('/', "\\"));
    #[cfg(not(windows))]
    let path = PathBuf::from(format!("/{decoded_path}"));

    Ok(normalize_path(&path))
}

pub(crate) fn uri_from_path(path: &Path) -> Result<Uri> {
    let normalized = normalize_path(path);
    let path_text = normalized.to_string_lossy().replace('\\', "/");

    #[cfg(windows)]
    let raw = format!("file:///{}", path_text.replace(' ', "%20"));
    #[cfg(not(windows))]
    let raw = format!("file://{}", path_text.replace(' ', "%20"));

    raw.parse::<Uri>().map_err(|error| {
        anyhow!(
            "failed to build file:// URI from `{}`: {error}",
            normalized.display()
        )
    })
}
