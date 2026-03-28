use std::path::Path;

use rhai_db::{AnalyzerDatabase, ChangeImpact, ChangeSet, DatabaseSnapshot};
use rhai_hir::{Symbol, SymbolKind, TypeRef};
use rhai_syntax::{TextRange, TextSize};
use rhai_vfs::FileId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FilePosition {
    pub file_id: FileId,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverResult {
    pub signature: String,
    pub docs: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub full_range: TextRange,
    pub focus_range: TextRange,
    pub children: Vec<DocumentSymbol>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSymbol {
    pub file_id: FileId,
    pub name: String,
    pub kind: SymbolKind,
    pub full_range: TextRange,
    pub focus_range: TextRange,
    pub container_name: Option<String>,
    pub exported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationTarget {
    pub file_id: FileId,
    pub kind: SymbolKind,
    pub full_range: TextRange,
    pub focus_range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceKind {
    Definition,
    Reference,
    LinkedImport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceLocation {
    pub file_id: FileId,
    pub range: TextRange,
    pub kind: ReferenceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferencesResult {
    pub targets: Vec<NavigationTarget>,
    pub references: Vec<ReferenceLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameIssue {
    pub file_id: FileId,
    pub message: String,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenamePlan {
    pub new_name: String,
    pub targets: Vec<NavigationTarget>,
    pub occurrences: Vec<ReferenceLocation>,
    pub issues: Vec<RenameIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionItemKind {
    Symbol(SymbolKind),
    Member,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionItemSource {
    Visible,
    Project,
    Member,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionItemKind,
    pub source: CompletionItemSource,
    pub detail: Option<String>,
    pub docs: Option<String>,
    pub file_id: Option<FileId>,
    pub exported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoImportAction {
    pub label: String,
    pub module_name: String,
    pub provider_file_id: FileId,
    pub insert_offset: u32,
    pub insert_text: String,
}

#[derive(Debug, Default)]
pub struct AnalysisHost {
    db: AnalyzerDatabase,
}

impl AnalysisHost {
    pub fn apply_change(&mut self, change_set: ChangeSet) {
        self.db.apply_change(change_set);
    }

    pub fn apply_change_report(&mut self, change_set: ChangeSet) -> ChangeImpact {
        self.db.apply_change_report(change_set)
    }

    pub fn warm_query_support(&mut self, file_ids: &[FileId]) -> usize {
        self.db.warm_query_support(file_ids)
    }

    pub fn set_query_support_budget(&mut self, budget: Option<usize>) -> Vec<FileId> {
        self.db.set_query_support_budget(budget)
    }

    pub fn snapshot(&self) -> Analysis {
        Analysis {
            db: self.db.snapshot(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Analysis {
    db: DatabaseSnapshot,
}

impl Analysis {
    pub fn file_id_for_path(&self, path: &Path) -> Option<FileId> {
        self.db.vfs().file_id(path)
    }

    pub fn normalized_path(&self, file_id: FileId) -> Option<&Path> {
        self.db.normalized_path(file_id)
    }

    pub fn has_query_support(&self, file_id: FileId) -> bool {
        self.db.query_support(file_id).is_some()
    }

    pub fn diagnostics(&self, file_id: FileId) -> Vec<Diagnostic> {
        if self.db.file_text(file_id).is_none() {
            return Vec::new();
        }

        self.db
            .project_diagnostics(file_id)
            .into_iter()
            .map(|diagnostic| Diagnostic {
                message: diagnostic.message,
                range: diagnostic.range,
            })
            .collect()
    }

    pub fn hover(&self, position: FilePosition) -> Option<HoverResult> {
        let target = self.goto_definition(position).into_iter().next()?;
        let hir = self.db.hir(target.file_id)?;
        let symbol = hir.symbol_at(target.full_range)?;
        let symbol = hir.symbol(symbol);
        let docs = symbol.docs.map(|docs| hir.doc_block(docs).text.clone());

        Some(HoverResult {
            signature: format_symbol_signature(
                symbol.name.as_str(),
                symbol.kind,
                symbol.annotation.as_ref(),
            ),
            docs,
        })
    }

    pub fn symbols(&self, file_id: FileId) -> Vec<Symbol> {
        self.db
            .hir(file_id)
            .map_or_else(Vec::new, |hir| hir.symbols.clone())
    }

    pub fn document_symbols(&self, file_id: FileId) -> Vec<DocumentSymbol> {
        self.db
            .document_symbols(file_id)
            .iter()
            .map(document_symbol_from_db)
            .collect()
    }

    pub fn workspace_symbols(&self) -> Vec<WorkspaceSymbol> {
        self.db
            .workspace_symbols()
            .iter()
            .map(workspace_symbol_from_db)
            .collect()
    }

    pub fn workspace_symbols_matching(&self, query: &str) -> Vec<WorkspaceSymbol> {
        self.db
            .workspace_symbols_matching(query)
            .iter()
            .map(workspace_symbol_from_db)
            .collect()
    }

    pub fn goto_definition(&self, position: FilePosition) -> Vec<NavigationTarget> {
        self.db
            .goto_definition(position.file_id, text_size(position.offset))
            .into_iter()
            .map(navigation_target_from_db)
            .collect()
    }

    pub fn find_references(&self, position: FilePosition) -> Option<ReferencesResult> {
        let result = self
            .db
            .find_references(position.file_id, text_size(position.offset))?;

        Some(ReferencesResult {
            targets: result
                .targets
                .iter()
                .map(navigation_target_from_identity)
                .collect(),
            references: result
                .references
                .iter()
                .map(reference_location_from_db)
                .collect(),
        })
    }

    pub fn rename_plan(
        &self,
        position: FilePosition,
        new_name: impl Into<String>,
    ) -> Option<RenamePlan> {
        let plan = self
            .db
            .rename_plan(position.file_id, text_size(position.offset), new_name)?;

        Some(RenamePlan {
            new_name: plan.new_name,
            targets: plan
                .targets
                .iter()
                .map(navigation_target_from_identity)
                .collect(),
            occurrences: plan
                .occurrences
                .iter()
                .map(reference_location_from_db)
                .collect(),
            issues: plan
                .issues
                .iter()
                .map(|issue| RenameIssue {
                    file_id: issue.file_id,
                    message: issue.issue.message.clone(),
                    range: issue.issue.range,
                })
                .collect(),
        })
    }

    pub fn completions(&self, position: FilePosition) -> Vec<CompletionItem> {
        let Some(inputs) = self
            .db
            .completion_inputs(position.file_id, text_size(position.offset))
        else {
            return Vec::new();
        };

        let mut items = Vec::new();
        let hir = self.db.hir(position.file_id);

        items.extend(inputs.visible_symbols.iter().map(|symbol| {
            let docs = match (&hir, symbol.docs) {
                (Some(hir), Some(docs)) => Some(hir.doc_block(docs).text.clone()),
                _ => None,
            };

            CompletionItem {
                label: symbol.name.clone(),
                kind: CompletionItemKind::Symbol(symbol.kind),
                source: CompletionItemSource::Visible,
                detail: symbol.annotation.as_ref().map(format_type_ref),
                docs,
                file_id: Some(position.file_id),
                exported: false,
            }
        }));

        items.extend(inputs.project_symbols.iter().map(|symbol| CompletionItem {
            label: symbol.symbol.name.clone(),
            kind: CompletionItemKind::Symbol(symbol.symbol.kind),
            source: CompletionItemSource::Project,
            detail: None,
            docs: None,
            file_id: Some(symbol.file_id),
            exported: symbol.symbol.exported,
        }));

        items.extend(inputs.member_symbols.iter().map(|member| CompletionItem {
            label: member.name.clone(),
            kind: CompletionItemKind::Member,
            source: CompletionItemSource::Member,
            detail: member.annotation.as_ref().map(format_type_ref),
            docs: None,
            file_id: None,
            exported: false,
        }));

        items
    }

    pub fn auto_import_actions(&self, position: FilePosition) -> Vec<AutoImportAction> {
        self.db
            .auto_import_candidates(position.file_id, text_size(position.offset))
            .into_iter()
            .map(|candidate| AutoImportAction {
                label: format!("Import `{}`", candidate.module_name),
                module_name: candidate.module_name,
                provider_file_id: candidate.provider_file_id,
                insert_offset: u32::from(candidate.insertion_offset),
                insert_text: candidate.insert_text,
            })
            .collect()
    }
}

fn document_symbol_from_db(symbol: &rhai_hir::DocumentSymbol) -> DocumentSymbol {
    DocumentSymbol {
        name: symbol.name.clone(),
        kind: symbol.kind,
        full_range: symbol.full_range,
        focus_range: symbol.focus_range,
        children: symbol
            .children
            .iter()
            .map(document_symbol_from_db)
            .collect(),
    }
}

fn workspace_symbol_from_db(symbol: &rhai_db::LocatedWorkspaceSymbol) -> WorkspaceSymbol {
    WorkspaceSymbol {
        file_id: symbol.file_id,
        name: symbol.symbol.name.clone(),
        kind: symbol.symbol.kind,
        full_range: symbol.symbol.full_range,
        focus_range: symbol.symbol.focus_range,
        container_name: symbol.symbol.container_name.clone(),
        exported: symbol.symbol.exported,
    }
}

fn navigation_target_from_db(target: rhai_db::LocatedNavigationTarget) -> NavigationTarget {
    NavigationTarget {
        file_id: target.file_id,
        kind: target.target.kind,
        full_range: target.target.full_range,
        focus_range: target.target.focus_range,
    }
}

fn navigation_target_from_identity(target: &rhai_db::LocatedSymbolIdentity) -> NavigationTarget {
    NavigationTarget {
        file_id: target.file_id,
        kind: target.symbol.kind,
        full_range: target.symbol.declaration_range,
        focus_range: target.symbol.declaration_range,
    }
}

fn reference_location_from_db(reference: &rhai_db::LocatedProjectReference) -> ReferenceLocation {
    ReferenceLocation {
        file_id: reference.file_id,
        range: reference.range,
        kind: match reference.kind {
            rhai_db::ProjectReferenceKind::Definition => ReferenceKind::Definition,
            rhai_db::ProjectReferenceKind::Reference => ReferenceKind::Reference,
            rhai_db::ProjectReferenceKind::LinkedImport => ReferenceKind::LinkedImport,
        },
    }
}

fn format_symbol_signature(name: &str, kind: SymbolKind, annotation: Option<&TypeRef>) -> String {
    match annotation {
        Some(TypeRef::Function(signature)) => format!(
            "fn {name}({}) -> {}",
            signature
                .params
                .iter()
                .map(format_type_ref)
                .collect::<Vec<_>>()
                .join(", "),
            format_type_ref(signature.ret.as_ref())
        ),
        Some(annotation) => match kind {
            SymbolKind::Constant => format!("const {name}: {}", format_type_ref(annotation)),
            SymbolKind::Parameter => format!("param {name}: {}", format_type_ref(annotation)),
            _ => format!("let {name}: {}", format_type_ref(annotation)),
        },
        None => match kind {
            SymbolKind::Function => format!("fn {name}"),
            SymbolKind::Constant => format!("const {name}"),
            SymbolKind::ImportAlias => format!("import {name}"),
            SymbolKind::ExportAlias => format!("export {name}"),
            SymbolKind::Parameter => format!("param {name}"),
            SymbolKind::Variable => format!("let {name}"),
        },
    }
}

fn format_type_ref(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Unknown => "unknown".to_owned(),
        TypeRef::Any => "any".to_owned(),
        TypeRef::Never => "never".to_owned(),
        TypeRef::Bool => "bool".to_owned(),
        TypeRef::Int => "int".to_owned(),
        TypeRef::Float => "float".to_owned(),
        TypeRef::String => "string".to_owned(),
        TypeRef::Char => "char".to_owned(),
        TypeRef::Named(name) => name.clone(),
        TypeRef::Applied { name, args } => format!(
            "{name}<{}>",
            args.iter()
                .map(format_type_ref)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeRef::Array(inner) => format!("array<{}>", format_type_ref(inner)),
        TypeRef::Map(key, value) => {
            format!("map<{}, {}>", format_type_ref(key), format_type_ref(value))
        }
        TypeRef::Nullable(inner) => format!("{}?", format_type_ref(inner)),
        TypeRef::Union(members) => members
            .iter()
            .map(format_type_ref)
            .collect::<Vec<_>>()
            .join(" | "),
        TypeRef::Function(signature) => format!(
            "fun({}) -> {}",
            signature
                .params
                .iter()
                .map(format_type_ref)
                .collect::<Vec<_>>()
                .join(", "),
            format_type_ref(signature.ret.as_ref())
        ),
    }
}

fn text_size(offset: u32) -> TextSize {
    TextSize::from(offset)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{AnalysisHost, FilePosition, ReferenceKind};
    use rhai_db::ChangeSet;
    use rhai_vfs::DocumentVersion;

    #[test]
    fn diagnostics_return_empty_for_missing_files() {
        let host = AnalysisHost::default();
        let analysis = host.snapshot();

        assert!(analysis.diagnostics(rhai_vfs::FileId(999)).is_empty());
    }

    #[test]
    fn document_symbols_use_database_indexes() {
        let mut host = AnalysisHost::default();
        host.apply_change(ChangeSet::single_file(
            "main.rhai",
            r#"
                fn outer() {
                    fn inner() {}
                }

                const LIMIT = 1;
                export outer as public_outer;
            "#,
            DocumentVersion(1),
        ));

        let analysis = host.snapshot();
        let file_id = analysis
            .db
            .vfs()
            .file_id(Path::new("main.rhai"))
            .expect("expected file id");
        let document_symbols = analysis.document_symbols(file_id);

        assert_eq!(
            document_symbols
                .iter()
                .map(|symbol| symbol.name.as_str())
                .collect::<Vec<_>>(),
            vec!["outer", "LIMIT", "public_outer"]
        );
        assert_eq!(document_symbols[0].children.len(), 1);
        assert_eq!(document_symbols[0].children[0].name, "inner");
    }

    #[test]
    fn workspace_symbols_include_file_identity() {
        let mut host = AnalysisHost::default();
        host.apply_change(ChangeSet {
            files: vec![
                rhai_db::FileChange {
                    path: "one.rhai".into(),
                    text: "fn alpha() {}".to_owned(),
                    version: DocumentVersion(1),
                },
                rhai_db::FileChange {
                    path: "two.rhai".into(),
                    text: "fn beta() {}".to_owned(),
                    version: DocumentVersion(1),
                },
            ],
            removed_files: Vec::new(),
            project: None,
        });

        let analysis = host.snapshot();
        let one = analysis
            .db
            .vfs()
            .file_id(Path::new("one.rhai"))
            .expect("expected one.rhai");
        let two = analysis
            .db
            .vfs()
            .file_id(Path::new("two.rhai"))
            .expect("expected two.rhai");

        assert_eq!(
            analysis
                .workspace_symbols()
                .iter()
                .map(|symbol| (symbol.file_id, symbol.name.as_str()))
                .collect::<Vec<_>>(),
            vec![(one, "alpha"), (two, "beta")]
        );
    }

    #[test]
    fn goto_definition_uses_cross_file_database_navigation() {
        let mut host = AnalysisHost::default();
        host.apply_change(ChangeSet {
            files: vec![
                rhai_db::FileChange {
                    path: "provider.rhai".into(),
                    text: "fn helper() {} export helper as shared_tools;".to_owned(),
                    version: DocumentVersion(1),
                },
                rhai_db::FileChange {
                    path: "consumer.rhai".into(),
                    text: "import shared_tools as tools;".to_owned(),
                    version: DocumentVersion(1),
                },
            ],
            removed_files: Vec::new(),
            project: None,
        });

        let analysis = host.snapshot();
        let provider = analysis
            .db
            .vfs()
            .file_id(Path::new("provider.rhai"))
            .expect("expected provider.rhai");
        let consumer = analysis
            .db
            .vfs()
            .file_id(Path::new("consumer.rhai"))
            .expect("expected consumer.rhai");
        let offset = u32::try_from(
            analysis
                .db
                .file_text(consumer)
                .expect("expected consumer text")
                .find("shared_tools")
                .expect("expected shared_tools"),
        )
        .expect("expected offset to fit");

        let targets = analysis.goto_definition(FilePosition {
            file_id: consumer,
            offset,
        });

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].file_id, provider);
    }

    #[test]
    fn references_and_rename_plan_surface_project_level_results() {
        let mut host = AnalysisHost::default();
        host.apply_change(ChangeSet {
            files: vec![
                rhai_db::FileChange {
                    path: "provider.rhai".into(),
                    text: r#"
                        fn helper() { helper(); }
                        export helper as shared_tools;
                    "#
                    .to_owned(),
                    version: DocumentVersion(1),
                },
                rhai_db::FileChange {
                    path: "consumer.rhai".into(),
                    text: "import shared_tools as tools;".to_owned(),
                    version: DocumentVersion(1),
                },
            ],
            removed_files: Vec::new(),
            project: None,
        });

        let analysis = host.snapshot();
        let consumer = analysis
            .db
            .vfs()
            .file_id(Path::new("consumer.rhai"))
            .expect("expected consumer.rhai");
        let consumer_offset = u32::try_from(
            analysis
                .db
                .file_text(consumer)
                .expect("expected consumer text")
                .find("shared_tools")
                .expect("expected shared_tools"),
        )
        .expect("expected offset to fit");

        let references = analysis
            .find_references(FilePosition {
                file_id: consumer,
                offset: consumer_offset,
            })
            .expect("expected references result");
        assert_eq!(references.targets.len(), 1);
        assert!(
            references
                .references
                .iter()
                .any(|reference| reference.kind == ReferenceKind::LinkedImport)
        );

        let rename = analysis
            .rename_plan(
                FilePosition {
                    file_id: consumer,
                    offset: consumer_offset,
                },
                "renamed_tools",
            )
            .expect("expected rename plan");
        assert_eq!(rename.new_name, "renamed_tools");
        assert!(
            rename
                .occurrences
                .iter()
                .any(|occurrence| occurrence.kind == ReferenceKind::LinkedImport)
        );
    }

    #[test]
    fn completions_merge_visible_project_and_member_results() {
        let mut host = AnalysisHost::default();
        host.apply_change(ChangeSet {
            files: vec![
                rhai_db::FileChange {
                    path: "main.rhai".into(),
                    text: r#"
                        /// helper docs
                        /// @type fun() -> bool
                        fn helper() {}

                        fn run() {
                            let user = #{ name: "Ada" };
                            user.
                            helper();
                        }
                    "#
                    .to_owned(),
                    version: DocumentVersion(1),
                },
                rhai_db::FileChange {
                    path: "support.rhai".into(),
                    text: "fn shared_helper() {}".to_owned(),
                    version: DocumentVersion(1),
                },
            ],
            removed_files: Vec::new(),
            project: None,
        });

        let analysis = host.snapshot();
        let main = analysis
            .db
            .vfs()
            .file_id(Path::new("main.rhai"))
            .expect("expected main.rhai");
        let main_text = analysis.db.file_text(main).expect("expected main text");

        let helper_offset =
            u32::try_from(main_text.find("helper();").expect("expected helper call"))
                .expect("expected offset to fit");
        let helper_completions = analysis.completions(FilePosition {
            file_id: main,
            offset: helper_offset,
        });
        assert!(helper_completions.iter().any(|item| {
            item.label == "helper" && item.source == super::CompletionItemSource::Visible
        }));
        assert!(helper_completions.iter().any(|item| {
            item.label == "shared_helper" && item.source == super::CompletionItemSource::Project
        }));

        let member_offset = u32::try_from(main_text.find("user.").expect("expected member access"))
            .expect("expected offset to fit");
        let member_completions = analysis.completions(FilePosition {
            file_id: main,
            offset: member_offset,
        });
        assert!(member_completions.iter().any(|item| {
            item.label == "name" && item.source == super::CompletionItemSource::Member
        }));
    }

    #[test]
    fn diagnostics_respect_workspace_linked_import_resolution() {
        let mut host = AnalysisHost::default();
        host.apply_change(ChangeSet {
            files: vec![
                rhai_db::FileChange {
                    path: "provider.rhai".into(),
                    text: "fn helper() {} export helper as shared_tools;".to_owned(),
                    version: DocumentVersion(1),
                },
                rhai_db::FileChange {
                    path: "consumer.rhai".into(),
                    text: "import shared_tools as tools;\n\nfn run() { tools(); }".to_owned(),
                    version: DocumentVersion(1),
                },
            ],
            removed_files: Vec::new(),
            project: None,
        });

        let analysis = host.snapshot();
        let consumer = analysis
            .db
            .vfs()
            .file_id(Path::new("consumer.rhai"))
            .expect("expected consumer.rhai");
        assert!(analysis.diagnostics(consumer).is_empty());

        host.apply_change(ChangeSet::single_file(
            "provider.rhai",
            "fn helper() {} export helper as renamed_tools;",
            DocumentVersion(2),
        ));

        let analysis = host.snapshot();
        let diagnostics = analysis.diagnostics(consumer);
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message == "unresolved import module `shared_tools`"
            })
        );
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("import alias no longer resolves")
        }));
    }

    #[test]
    fn auto_import_actions_plan_insert_for_unresolved_workspace_exports() {
        let mut host = AnalysisHost::default();
        host.apply_change(ChangeSet {
            files: vec![
                rhai_db::FileChange {
                    path: "provider.rhai".into(),
                    text: "fn helper() {} export helper as shared_tools;".to_owned(),
                    version: DocumentVersion(1),
                },
                rhai_db::FileChange {
                    path: "consumer.rhai".into(),
                    text: "fn run() { shared_tools(); }".to_owned(),
                    version: DocumentVersion(1),
                },
            ],
            removed_files: Vec::new(),
            project: None,
        });

        let analysis = host.snapshot();
        let consumer = analysis
            .db
            .vfs()
            .file_id(Path::new("consumer.rhai"))
            .expect("expected consumer.rhai");
        let offset = u32::try_from(
            analysis
                .db
                .file_text(consumer)
                .expect("expected consumer text")
                .find("shared_tools")
                .expect("expected unresolved reference"),
        )
        .expect("expected offset to fit");

        let actions = analysis.auto_import_actions(FilePosition {
            file_id: consumer,
            offset,
        });

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].module_name, "shared_tools");
        assert_eq!(actions[0].insert_offset, 0);
        assert_eq!(
            actions[0].insert_text,
            "import shared_tools as shared_tools;\n"
        );
    }
}
