use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use lsp_types::{
    CodeActionProviderCapability, CompletionOptions, HoverProviderCapability, InitializeResult,
    OneOf, ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
};
use rhai_db::{ChangeImpact, ChangeSet};
use rhai_ide::{AnalysisHost, AutoImportAction, Diagnostic, FilePosition};
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
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeActionEdit {
    pub title: String,
    pub uri: Uri,
    pub version: Option<i32>,
    pub insert_offset: u32,
    pub insert_text: String,
}

#[derive(Debug)]
pub struct Server {
    analysis_host: AnalysisHost,
    open_documents: HashMap<Uri, ManagedDocument>,
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

    pub fn capabilities(&self) -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            definition_provider: Some(OneOf::Left(true)),
            references_provider: Some(OneOf::Left(true)),
            rename_provider: Some(OneOf::Left(true)),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            document_symbol_provider: Some(OneOf::Left(true)),
            completion_provider: Some(CompletionOptions::default()),
            code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
            ..ServerCapabilities::default()
        }
    }

    pub fn initialize_result(&self) -> InitializeResult {
        InitializeResult {
            capabilities: self.capabilities(),
            server_info: Some(ServerInfo {
                name: "rhai-lsp".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
        }
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

    pub fn close_document(&mut self, uri: &Uri) -> Vec<DiagnosticUpdate> {
        let Some(document) = self.open_documents.remove(uri) else {
            return Vec::new();
        };

        let impact = self
            .analysis_host
            .apply_change_report(ChangeSet::remove_file(document.normalized_path.clone()));
        let mut updates = vec![DiagnosticUpdate {
            uri: document.uri,
            version: None,
            diagnostics: Vec::new(),
        }];
        updates.extend(self.diagnostic_updates_for_impact(&impact));
        dedupe_diagnostic_updates(updates)
    }

    pub fn auto_import_actions(&self, uri: &Uri, offset: u32) -> Result<Vec<CodeActionEdit>> {
        let uri_text = uri.as_str();
        let document = self
            .open_documents
            .get(uri)
            .ok_or_else(|| anyhow!("document `{uri_text}` is not open"))?;
        let analysis = self.analysis_host.snapshot();
        let file_id = analysis
            .file_id_for_path(&document.normalized_path)
            .ok_or_else(|| anyhow!("document `{uri_text}` is not loaded in the analysis host"))?;

        Ok(analysis
            .auto_import_actions(FilePosition { file_id, offset })
            .into_iter()
            .filter_map(|action| code_action_edit_from_ide(uri, document.version, action))
            .collect())
    }

    fn upsert_document(
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

    fn warm_hot_files(&mut self, impact: &ChangeImpact) {
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

    fn diagnostic_updates_for_impact(&self, impact: &ChangeImpact) -> Vec<DiagnosticUpdate> {
        let analysis = self.analysis_host.snapshot();
        let mut target_files = BTreeSet::<FileId>::new();

        for &file_id in &impact.rebuilt_files {
            target_files.insert(file_id);
        }
        for &file_id in &impact.dependency_affected_files {
            target_files.insert(file_id);
        }

        let path_to_document = self
            .open_documents
            .values()
            .map(|document| (document.normalized_path.clone(), document))
            .collect::<BTreeMap<_, _>>();

        let mut updates = Vec::new();
        for file_id in target_files {
            let Some(path) = analysis.normalized_path(file_id) else {
                continue;
            };
            let Some(document) = path_to_document.get(path) else {
                continue;
            };

            updates.push(DiagnosticUpdate {
                uri: document.uri.clone(),
                version: Some(document.version),
                diagnostics: analysis.diagnostics(file_id),
            });
        }

        updates
    }
}

fn path_from_uri(uri: &Uri) -> Result<PathBuf> {
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

fn dedupe_diagnostic_updates(updates: Vec<DiagnosticUpdate>) -> Vec<DiagnosticUpdate> {
    let mut by_uri = BTreeMap::<String, DiagnosticUpdate>::new();
    for update in updates {
        by_uri.insert(update.uri.to_string(), update);
    }
    by_uri.into_values().collect()
}

fn code_action_edit_from_ide(
    uri: &Uri,
    version: i32,
    action: AutoImportAction,
) -> Option<CodeActionEdit> {
    let [file_edit] = action.source_change.file_edits.as_slice() else {
        return None;
    };
    let [edit] = file_edit.edits.as_slice() else {
        return None;
    };
    let insert_offset = edit.insertion_offset()?;

    Some(CodeActionEdit {
        title: action.label,
        uri: uri.clone(),
        version: Some(version),
        insert_offset,
        insert_text: edit.new_text.clone(),
    })
}

#[cfg(test)]
mod tests {
    use lsp_types::Uri;
    use rhai_syntax::parse_text;

    use super::Server;

    #[test]
    fn opening_document_returns_diagnostics_for_that_document() {
        let mut server = Server::new();
        let uri = file_url("main.rhai");

        let updates = server
            .open_document(uri.clone(), 1, "let value = ;")
            .expect("expected open_document to succeed");

        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].uri, uri);
        assert_eq!(updates[0].version, Some(1));
        assert!(!updates[0].diagnostics.is_empty());
    }

    #[test]
    fn changing_document_republishes_open_dependents_and_warms_hot_files() {
        let mut server = Server::new();
        let provider_uri = file_url("provider.rhai");
        let consumer_uri = file_url("consumer.rhai");
        let provider_text = "export const VALUE = 1;";
        let consumer_text = "import \"provider\" as tools;\ntools;\nfn run() {}";
        let renamed_provider_text = "export const VALUE = 2;";

        assert_valid_rhai_syntax(provider_text);
        assert_valid_rhai_syntax(consumer_text);
        assert_valid_rhai_syntax(renamed_provider_text);

        server
            .open_document(provider_uri.clone(), 1, provider_text)
            .expect("expected provider open to succeed");
        server
            .open_document(consumer_uri.clone(), 1, consumer_text)
            .expect("expected consumer open to succeed");

        let updates = server
            .change_document(provider_uri.clone(), 2, renamed_provider_text)
            .expect("expected provider change to succeed");

        assert!(updates.iter().any(|update| update.uri == provider_uri));
        let consumer_update = updates
            .iter()
            .find(|update| update.uri == consumer_uri)
            .expect("expected consumer diagnostics update");
        assert!(consumer_update.diagnostics.is_empty());

        let analysis = server.analysis_host().snapshot();
        let provider = analysis
            .file_id_for_path(&absolute_test_path("provider.rhai"))
            .expect("expected provider file id");
        let consumer = analysis
            .file_id_for_path(&absolute_test_path("consumer.rhai"))
            .expect("expected consumer file id");
        assert!(analysis.has_query_support(provider));
        assert!(analysis.has_query_support(consumer));
    }

    #[test]
    fn closing_document_clears_diagnostics_and_unloads_file() {
        let mut server = Server::new();
        let uri = file_url("main.rhai");

        server
            .open_document(uri.clone(), 1, "let value = ;")
            .expect("expected open_document to succeed");
        let updates = server.close_document(&uri);

        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].uri, uri);
        assert!(updates[0].diagnostics.is_empty());
        assert_eq!(updates[0].version, None);

        let analysis = server.analysis_host().snapshot();
        assert!(
            analysis
                .file_id_for_path(&absolute_test_path("main.rhai"))
                .is_none()
        );
    }

    #[test]
    fn auto_import_actions_are_not_exposed_for_workspace_exports() {
        let mut server = Server::new();
        let provider_uri = file_url("provider.rhai");
        let consumer_uri = file_url("consumer.rhai");
        let provider_text = "let helper = 1; export helper as shared_tools;";
        let consumer_text = "fn run() { shared_tools(); }";

        assert_valid_rhai_syntax(provider_text);
        assert_valid_rhai_syntax(consumer_text);

        server
            .open_document(provider_uri, 1, provider_text)
            .expect("expected provider open to succeed");
        server
            .open_document(consumer_uri.clone(), 1, consumer_text)
            .expect("expected consumer open to succeed");

        let actions = server
            .auto_import_actions(
                &consumer_uri,
                offset_in("fn run() { shared_tools(); }", "shared_tools"),
            )
            .expect("expected auto import actions");

        assert!(actions.is_empty());
    }

    fn file_url(path: &str) -> Uri {
        let absolute = absolute_test_path(path);
        let raw = format!("file:///{}", absolute.display()).replace('\\', "/");
        raw.parse::<Uri>().expect("expected file URI")
    }

    fn absolute_test_path(path: &str) -> std::path::PathBuf {
        std::env::current_dir()
            .expect("expected current dir")
            .join(path)
    }

    fn offset_in(text: &str, needle: &str) -> u32 {
        u32::try_from(text.find(needle).expect("expected needle")).expect("expected offset")
    }

    fn assert_valid_rhai_syntax(text: &str) {
        let parse = parse_text(text);
        assert!(
            parse.errors().is_empty(),
            "expected valid Rhai syntax, got errors: {:?}",
            parse.errors()
        );
    }
}
