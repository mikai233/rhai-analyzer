use anyhow::{Result, anyhow};
use lsp_types::Uri;
use rhai_ide::{AutoImportAction, FilePosition};

use crate::server::{CodeActionEdit, Server};

impl Server {
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
