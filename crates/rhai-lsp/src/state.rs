use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use lsp_types::{
    SemanticToken, SemanticTokens, SemanticTokensDelta, SemanticTokensEdit,
    SemanticTokensFullDeltaResult, Uri,
};
use rhai_db::{ChangeImpact, ChangeSet, FileChange};
use rhai_fmt::{ContainerLayoutStyle, ImportSortOrder};
use rhai_ide::AnalysisHost;
use rhai_syntax::{AstNode, Expr, Item, Root, Stmt, TokenKind, parse_text};
use rhai_vfs::{DocumentVersion, FileId, normalize_path};
use url::Url;

const DEFAULT_QUERY_SUPPORT_BUDGET: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedDocument {
    pub uri: Uri,
    pub normalized_path: PathBuf,
    pub version: i32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticUpdate {
    pub uri: Uri,
    pub version: Option<i32>,
    pub diagnostics: Vec<rhai_ide::Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceLoadResult {
    pub file_count: usize,
    pub updates: Vec<DiagnosticUpdate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSymbolMatch {
    pub uri: Uri,
    pub symbol: rhai_ide::WorkspaceSymbol,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SemanticTokenCacheEntry {
    result_id: String,
    data: Vec<SemanticToken>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InlayHintSettings {
    pub variables: bool,
    pub parameters: bool,
    pub return_types: bool,
}

impl Default for InlayHintSettings {
    fn default() -> Self {
        Self {
            variables: true,
            parameters: true,
            return_types: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatterSettings {
    pub max_line_length: usize,
    pub trailing_commas: bool,
    pub final_newline: bool,
    pub container_layout: ContainerLayoutStyle,
    pub import_sort_order: ImportSortOrder,
}

impl Default for FormatterSettings {
    fn default() -> Self {
        Self {
            max_line_length: 100,
            trailing_commas: true,
            final_newline: true,
            container_layout: ContainerLayoutStyle::Auto,
            import_sort_order: ImportSortOrder::Preserve,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ServerSettings {
    pub inlay_hints: InlayHintSettings,
    pub formatter: FormatterSettings,
}

#[derive(Debug)]
pub struct ServerState {
    pub(crate) analysis_host: AnalysisHost,
    pub(crate) open_documents: HashMap<Uri, ManagedDocument>,
    semantic_token_cache: HashMap<Uri, SemanticTokenCacheEntry>,
    pub(crate) supports_work_done_progress: bool,
    pub(crate) next_server_request_id: u32,
    next_semantic_token_result_id: u64,
    pub(crate) workspace_roots: Vec<PathBuf>,
    pub(crate) preloaded_files: BTreeSet<PathBuf>,
    pub(crate) settings: ServerSettings,
}

pub type Server = ServerState;

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerState {
    pub fn new() -> Self {
        let mut analysis_host = AnalysisHost::default();
        let _ = analysis_host.set_query_support_budget(Some(DEFAULT_QUERY_SUPPORT_BUDGET));

        Self {
            analysis_host,
            open_documents: HashMap::new(),
            semantic_token_cache: HashMap::new(),
            supports_work_done_progress: false,
            next_server_request_id: 1,
            next_semantic_token_result_id: 1,
            workspace_roots: Vec::new(),
            preloaded_files: BTreeSet::new(),
            settings: ServerSettings::default(),
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

    pub(crate) fn uri_for_path(&self, path: &Path) -> Result<Uri> {
        let normalized = normalize_path(path);
        if let Some(document) = self
            .open_documents
            .values()
            .find(|document| document.normalized_path == normalized)
        {
            return Ok(document.uri.clone());
        }

        uri_from_path(&normalized)
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

    pub fn configure_client_capabilities(&mut self, supports_work_done_progress: bool) {
        self.supports_work_done_progress = supports_work_done_progress;
    }

    pub fn configure_settings(&mut self, settings: ServerSettings) {
        self.settings = settings;
    }

    pub fn settings(&self) -> ServerSettings {
        self.settings
    }

    pub fn supports_work_done_progress(&self) -> bool {
        self.supports_work_done_progress
    }

    pub fn next_server_request_id(&mut self) -> i32 {
        let id = self.next_server_request_id;
        self.next_server_request_id += 1;
        i32::try_from(id).unwrap_or(i32::MAX)
    }

    pub(crate) fn semantic_tokens_full(
        &mut self,
        uri: &Uri,
        mut tokens: SemanticTokens,
    ) -> SemanticTokens {
        let result_id = self.next_semantic_token_result_id.to_string();
        self.next_semantic_token_result_id += 1;

        self.semantic_token_cache.insert(
            uri.clone(),
            SemanticTokenCacheEntry {
                result_id: result_id.clone(),
                data: tokens.data.clone(),
            },
        );
        tokens.result_id = Some(result_id);
        tokens
    }

    pub(crate) fn semantic_tokens_delta(
        &mut self,
        uri: &Uri,
        previous_result_id: &str,
        tokens: SemanticTokens,
    ) -> SemanticTokensFullDeltaResult {
        let Some(previous) = self.semantic_token_cache.get(uri).cloned() else {
            return SemanticTokensFullDeltaResult::Tokens(self.semantic_tokens_full(uri, tokens));
        };

        if previous.result_id != previous_result_id {
            return SemanticTokensFullDeltaResult::Tokens(self.semantic_tokens_full(uri, tokens));
        }

        let result_id = self.next_semantic_token_result_id.to_string();
        self.next_semantic_token_result_id += 1;
        let edits = semantic_token_edits(&previous.data, &tokens.data);

        self.semantic_token_cache.insert(
            uri.clone(),
            SemanticTokenCacheEntry {
                result_id: result_id.clone(),
                data: tokens.data,
            },
        );

        SemanticTokensFullDeltaResult::TokensDelta(SemanticTokensDelta {
            result_id: Some(result_id),
            edits,
        })
    }

    pub(crate) fn clear_semantic_token_cache(&mut self, uri: &Uri) {
        self.semantic_token_cache.remove(uri);
    }

    pub fn load_workspace_roots(&mut self, roots: &[PathBuf]) -> Result<WorkspaceLoadResult> {
        self.workspace_roots = normalized_workspace_roots(roots);
        self.refresh_preloaded_files()
    }

    pub fn update_workspace_folders(
        &mut self,
        added: &[PathBuf],
        removed: &[PathBuf],
    ) -> Result<WorkspaceLoadResult> {
        let removed = removed
            .iter()
            .map(|root| normalize_path(root))
            .collect::<BTreeSet<_>>();
        let mut next_roots = self
            .workspace_roots
            .iter()
            .filter(|root| !removed.contains(*root))
            .cloned()
            .collect::<Vec<_>>();
        next_roots.extend(added.iter().cloned());
        self.workspace_roots = normalized_workspace_roots(&next_roots);
        self.refresh_preloaded_files()
    }

    pub fn reload_workspace_file(&mut self, uri: &Uri) -> Result<Vec<DiagnosticUpdate>> {
        if self.open_documents.contains_key(uri) {
            return Ok(Vec::new());
        }

        Ok(self.refresh_preloaded_files()?.updates)
    }

    pub fn remove_workspace_file(&mut self, uri: &Uri) -> Result<Vec<DiagnosticUpdate>> {
        if self.open_documents.contains_key(uri) {
            return Ok(Vec::new());
        }

        Ok(self.refresh_preloaded_files()?.updates)
    }

    pub fn rename_workspace_file(
        &mut self,
        old_uri: &Uri,
        new_uri: &Uri,
    ) -> Result<Vec<DiagnosticUpdate>> {
        let old_path = path_from_uri(old_uri)?;
        let new_path = path_from_uri(new_uri)?;
        let snapshot = self.analysis_host.snapshot();
        let text = snapshot
            .file_id_for_path(&old_path)
            .and_then(|file_id| snapshot.file_text(file_id))
            .map(|text| text.to_string())
            .or_else(|| fs::read_to_string(&new_path).ok());

        let Some(text) = text else {
            return Ok(self.refresh_preloaded_files()?.updates);
        };

        if let Some(mut document) = self.open_documents.remove(old_uri) {
            document.uri = new_uri.clone();
            document.normalized_path = new_path.clone();
            self.open_documents.insert(new_uri.clone(), document);
        }
        self.clear_semantic_token_cache(old_uri);
        self.clear_semantic_token_cache(new_uri);

        let version = self
            .open_documents
            .get(new_uri)
            .map(|document| DocumentVersion(document.version))
            .unwrap_or(DocumentVersion(0));

        let impact = self.analysis_host.apply_change_report(ChangeSet {
            files: vec![FileChange {
                path: new_path,
                text,
                version,
            }],
            removed_files: vec![old_path],
            project: None,
        });
        self.warm_hot_files(&impact);

        let mut updates = self.diagnostic_updates_for_impact(&impact);
        updates.extend(self.refresh_preloaded_files()?.updates);
        let new_version = self
            .open_documents
            .get(new_uri)
            .map(|document| document.version);
        for update in &mut updates {
            if update.uri == *old_uri {
                update.uri = new_uri.clone();
                if new_version.is_some() {
                    update.version = new_version;
                }
            }
        }
        updates.push(DiagnosticUpdate {
            uri: old_uri.clone(),
            version: None,
            diagnostics: Vec::new(),
        });
        Ok(dedupe_diagnostic_updates(updates))
    }

    pub(crate) fn upsert_document(
        &mut self,
        uri: Uri,
        version: i32,
        text: impl Into<String>,
    ) -> Result<Vec<DiagnosticUpdate>> {
        let had_document = self.open_documents.contains_key(&uri);
        let normalized_path = path_from_uri(&uri)?;
        let managed_document = ManagedDocument {
            uri: uri.clone(),
            normalized_path: normalized_path.clone(),
            version,
            text: text.into(),
        };
        if !had_document {
            self.clear_semantic_token_cache(&uri);
        }

        let impact = self
            .analysis_host
            .apply_change_report(ChangeSet::single_file(
                normalized_path,
                managed_document.text.clone(),
                DocumentVersion(version),
            ));
        self.open_documents.insert(uri, managed_document);
        self.warm_hot_files(&impact);
        let mut updates = self.diagnostic_updates_for_impact(&impact);
        updates.extend(self.refresh_preloaded_files()?.updates);
        Ok(dedupe_diagnostic_updates(updates))
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

    pub(crate) fn refresh_preloaded_files(&mut self) -> Result<WorkspaceLoadResult> {
        let snapshot = self.analysis_host.snapshot();
        let open_paths = self
            .open_documents
            .values()
            .map(|document| document.normalized_path.clone())
            .collect::<BTreeSet<_>>();
        let graph_files = preload_graph_files(&self.workspace_roots, &open_paths, &snapshot)?;

        let mut files = Vec::<FileChange>::new();
        let mut next_preloaded = BTreeSet::<PathBuf>::new();

        for (path, text) in graph_files.iter() {
            if open_paths.contains(path) {
                continue;
            }
            next_preloaded.insert(path.clone());
            files.push(FileChange {
                path: path.clone(),
                text: text.clone(),
                version: DocumentVersion(0),
            });
        }

        let removed_files = self
            .preloaded_files
            .difference(&next_preloaded)
            .filter(|path| !open_paths.contains(*path))
            .cloned()
            .collect::<Vec<_>>();

        let impact = self.analysis_host.apply_change_report(ChangeSet {
            files,
            removed_files,
            project: None,
        });
        self.preloaded_files = next_preloaded;
        self.warm_hot_files(&impact);

        Ok(WorkspaceLoadResult {
            file_count: graph_files.len(),
            updates: self.diagnostic_updates_for_impact(&impact),
        })
    }
}

pub(crate) fn path_from_uri(uri: &Uri) -> Result<PathBuf> {
    let raw = uri.as_str();
    let url = Url::parse(raw).map_err(|error| anyhow!("invalid URI `{raw}`: {error}"))?;
    let path = url
        .to_file_path()
        .map_err(|()| anyhow!("expected a file:// URI, got `{raw}`"))?;
    Ok(normalize_path(&path))
}

pub(crate) fn uri_from_path(path: &Path) -> Result<Uri> {
    let normalized = normalize_path(path);
    let raw: String = Url::from_file_path(&normalized)
        .map_err(|()| {
            anyhow!(
                "failed to build file:// URI from `{}`",
                normalized.display()
            )
        })?
        .into();
    raw.parse::<Uri>().map_err(|error| {
        anyhow!(
            "failed to build file:// URI from `{}`: {error}",
            normalized.display()
        )
    })
}

fn workspace_file_paths(roots: &[PathBuf]) -> Result<BTreeSet<PathBuf>> {
    let mut paths = BTreeSet::<PathBuf>::new();
    for root in roots {
        collect_rhai_files(root, &mut paths)?;
    }

    Ok(paths)
}

fn normalized_workspace_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots
        .iter()
        .map(|root| normalize_path(root))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn collect_rhai_files(root: &Path, paths: &mut BTreeSet<PathBuf>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }

    if root.is_file() {
        if root
            .extension()
            .is_some_and(|extension| extension == "rhai")
        {
            paths.insert(normalize_path(root));
        }
        return Ok(());
    }

    for entry in fs::read_dir(root).map_err(|error| {
        anyhow!(
            "failed to read workspace directory `{}`: {error}",
            root.display()
        )
    })? {
        let entry = entry.map_err(|error| {
            anyhow!(
                "failed to read an entry under `{}`: {error}",
                root.display()
            )
        })?;
        let path = normalize_path(&entry.path());
        let file_type = entry.file_type().map_err(|error| {
            anyhow!(
                "failed to inspect `{}` while scanning the workspace: {error}",
                path.display()
            )
        })?;

        if file_type.is_dir() {
            collect_rhai_files(&path, paths)?;
            continue;
        }

        if file_type.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "rhai")
        {
            paths.insert(path);
        }
    }

    Ok(())
}

fn preload_graph_files(
    workspace_roots: &[PathBuf],
    open_paths: &BTreeSet<PathBuf>,
    snapshot: &rhai_ide::Analysis,
) -> Result<BTreeMap<PathBuf, String>> {
    let mut seed_paths = workspace_file_paths(workspace_roots)?;
    for path in open_paths {
        seed_paths.insert(path.clone());
    }

    let mut visited = BTreeSet::<PathBuf>::new();
    let mut queue = seed_paths.into_iter().collect::<VecDeque<_>>();
    let mut files = BTreeMap::<PathBuf, String>::new();

    while let Some(path) = queue.pop_front() {
        let path = normalize_path(&path);
        if !visited.insert(path.clone()) {
            continue;
        }

        let Some(text) = graph_file_text(&path, open_paths, snapshot)? else {
            continue;
        };
        for import_path in static_import_dependencies(&path, &text) {
            if !visited.contains(&import_path) {
                queue.push_back(import_path);
            }
        }
        files.insert(path, text);
    }

    Ok(files)
}

fn graph_file_text(
    path: &Path,
    open_paths: &BTreeSet<PathBuf>,
    snapshot: &rhai_ide::Analysis,
) -> Result<Option<String>> {
    let normalized_path = normalize_path(path);
    if open_paths.contains(&normalized_path) {
        return Ok(snapshot
            .file_id_for_path(&normalized_path)
            .and_then(|file_id| snapshot.file_text(file_id))
            .map(|text| text.to_string()));
    }

    if !normalized_path.exists() {
        return Ok(None);
    }

    let text = fs::read_to_string(&normalized_path).map_err(|error| {
        anyhow!(
            "failed to read workspace-linked file `{}`: {error}",
            normalized_path.display()
        )
    })?;
    Ok(Some(text))
}

fn static_import_dependencies(importer_path: &Path, text: &str) -> Vec<PathBuf> {
    let parse = parse_text(text);
    let Some(root) = Root::cast(parse.root()) else {
        return Vec::new();
    };

    let mut dependencies = Vec::<PathBuf>::new();
    if let Some(items) = root.item_list() {
        for item in items.items() {
            let Item::Stmt(Stmt::Import(import_stmt)) = item else {
                continue;
            };
            let Some(module_expr) = import_stmt.module() else {
                continue;
            };
            let Some(module_name) = static_import_module_name(module_expr, parse.text()) else {
                continue;
            };
            if let Some(path) = candidate_module_paths(importer_path, &module_name)
                .into_iter()
                .find(|candidate| candidate.is_file())
                && !dependencies.iter().any(|dependency| dependency == &path)
            {
                dependencies.push(path);
            }
        }
    }

    dependencies
}

fn static_import_module_name(expr: Expr, _source: &str) -> Option<String> {
    let Expr::Literal(literal) = expr else {
        return None;
    };
    let token = literal.token()?;
    let text = token.text();

    match token.kind().token_kind() {
        Some(TokenKind::String) => text
            .strip_prefix('"')
            .and_then(|text| text.strip_suffix('"'))
            .map(str::to_owned),
        Some(TokenKind::BacktickString) => text
            .strip_prefix('`')
            .and_then(|text| text.strip_suffix('`'))
            .map(str::to_owned),
        _ => None,
    }
}

fn candidate_module_paths(importer_path: &Path, module_name: &str) -> Vec<PathBuf> {
    let relative = module_path_with_extension(module_name);
    let importer_dir = importer_path.parent().unwrap_or_else(|| Path::new(""));

    let mut candidates = vec![normalize_path(&importer_dir.join(&relative))];
    let direct = normalize_path(&relative);
    if !candidates.iter().any(|candidate| candidate == &direct) {
        candidates.push(direct);
    }
    candidates
}

fn module_path_with_extension(module_name: &str) -> PathBuf {
    let mut path = PathBuf::from(module_name);
    if path.extension().is_none() {
        path.set_extension("rhai");
    }
    path
}

fn dedupe_diagnostic_updates(updates: Vec<DiagnosticUpdate>) -> Vec<DiagnosticUpdate> {
    let mut by_uri = BTreeMap::<String, DiagnosticUpdate>::new();
    for update in updates {
        by_uri.insert(update.uri.to_string(), update);
    }
    by_uri.into_values().collect()
}

fn semantic_token_edits(
    previous: &[SemanticToken],
    next: &[SemanticToken],
) -> Vec<SemanticTokensEdit> {
    let mut prefix = 0usize;
    while prefix < previous.len() && prefix < next.len() && previous[prefix] == next[prefix] {
        prefix += 1;
    }

    let mut previous_suffix = previous.len();
    let mut next_suffix = next.len();
    while previous_suffix > prefix
        && next_suffix > prefix
        && previous[previous_suffix - 1] == next[next_suffix - 1]
    {
        previous_suffix -= 1;
        next_suffix -= 1;
    }

    let replacement = next[prefix..next_suffix].to_vec();
    vec![SemanticTokensEdit {
        start: (prefix as u32) * 5,
        delete_count: ((previous_suffix - prefix) as u32) * 5,
        data: (!replacement.is_empty()).then_some(replacement),
    }]
}
