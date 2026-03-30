use std::collections::{BTreeMap, BTreeSet};

use lsp_types::Uri;
use rhai_db::{ChangeImpact, ChangeSet};
use rhai_vfs::FileId;

use crate::state::{DiagnosticUpdate, ServerState};

impl ServerState {
    pub fn close_document(&mut self, uri: &Uri) -> Vec<DiagnosticUpdate> {
        let Some(document) = self.open_documents.remove(uri) else {
            return Vec::new();
        };

        let impact = self
            .analysis_host
            .apply_change_report(ChangeSet::remove_file(document.normalized_path.clone()));
        let mut updates = self.diagnostic_updates_for_impact(&impact);
        let reload_updates = self
            .refresh_preloaded_files()
            .map(|result| result.updates)
            .unwrap_or_default();
        updates.extend(reload_updates);

        let is_still_loaded = self
            .analysis_host
            .snapshot()
            .file_id_for_path(&document.normalized_path)
            .is_some();
        if !is_still_loaded {
            updates.push(DiagnosticUpdate {
                uri: document.uri,
                version: None,
                diagnostics: Vec::new(),
            });
        }

        dedupe_diagnostic_updates(updates)
    }

    pub(crate) fn diagnostic_updates_for_impact(
        &self,
        impact: &ChangeImpact,
    ) -> Vec<DiagnosticUpdate> {
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
            let (uri, version) = match path_to_document.get(path) {
                Some(document) => (document.uri.clone(), Some(document.version)),
                None => match self.uri_for_path(path) {
                    Ok(uri) => (uri, None),
                    Err(_) => continue,
                },
            };

            updates.push(DiagnosticUpdate {
                uri,
                version,
                diagnostics: analysis.diagnostics(file_id),
            });
        }

        updates
    }
}

pub(crate) fn dedupe_diagnostic_updates(updates: Vec<DiagnosticUpdate>) -> Vec<DiagnosticUpdate> {
    let mut by_uri = BTreeMap::<String, DiagnosticUpdate>::new();
    for update in updates {
        by_uri.insert(update.uri.to_string(), update);
    }
    by_uri.into_values().collect()
}
