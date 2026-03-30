use rhai_hir::{SymbolKind, TypeRef};
use rhai_syntax::TextSize;

use crate::{
    CallHierarchyItem, DocumentSymbol, NavigationTarget, ReferenceKind, ReferenceLocation,
    WorkspaceSymbol,
};

pub(crate) fn document_symbol_from_db(symbol: &rhai_hir::DocumentSymbol) -> DocumentSymbol {
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

pub(crate) fn workspace_symbol_from_db(
    symbol: &rhai_db::LocatedWorkspaceSymbol,
) -> WorkspaceSymbol {
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

pub(crate) fn navigation_target_from_db(
    target: rhai_db::LocatedNavigationTarget,
) -> NavigationTarget {
    NavigationTarget {
        file_id: target.file_id,
        kind: target.target.kind,
        full_range: target.target.full_range,
        focus_range: target.target.focus_range,
    }
}

pub(crate) fn navigation_target_from_identity(
    target: &rhai_db::LocatedSymbolIdentity,
) -> NavigationTarget {
    NavigationTarget {
        file_id: target.file_id,
        kind: target.symbol.kind,
        full_range: target.symbol.declaration_range,
        focus_range: target.symbol.declaration_range,
    }
}

pub(crate) fn call_hierarchy_item_from_db(
    item: &rhai_db::LocatedCallHierarchyItem,
) -> CallHierarchyItem {
    CallHierarchyItem {
        file_id: item.file_id,
        name: item.symbol.name.clone(),
        kind: item.target.kind,
        full_range: item.target.full_range,
        focus_range: item.target.focus_range,
        container_name: item.symbol.stable_key.container_path.last().cloned(),
    }
}

pub(crate) fn reference_location_from_db(
    reference: &rhai_db::LocatedProjectReference,
) -> ReferenceLocation {
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

pub(crate) fn format_symbol_signature(
    name: &str,
    kind: SymbolKind,
    annotation: Option<&TypeRef>,
) -> String {
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

pub(crate) fn format_type_ref(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Unknown => "unknown".to_owned(),
        TypeRef::Any => "any".to_owned(),
        TypeRef::Never => "never".to_owned(),
        TypeRef::Dynamic => "Dynamic".to_owned(),
        TypeRef::Bool => "bool".to_owned(),
        TypeRef::Int => "int".to_owned(),
        TypeRef::Float => "float".to_owned(),
        TypeRef::Decimal => "decimal".to_owned(),
        TypeRef::String => "string".to_owned(),
        TypeRef::Char => "char".to_owned(),
        TypeRef::Blob => "blob".to_owned(),
        TypeRef::Timestamp => "timestamp".to_owned(),
        TypeRef::FnPtr => "Fn".to_owned(),
        TypeRef::Unit => "()".to_owned(),
        TypeRef::Range => "range".to_owned(),
        TypeRef::RangeInclusive => "range=".to_owned(),
        TypeRef::Named(name) => name.clone(),
        TypeRef::Applied { name, args } => format!(
            "{name}<{}>",
            args.iter()
                .map(format_type_ref)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeRef::Object(fields) => format!(
            "#{{ {} }}",
            fields
                .iter()
                .map(|(name, ty)| format!("{name}: {}", format_type_ref(ty)))
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
        TypeRef::Ambiguous(members) => format!(
            "ambiguous<{}>",
            members
                .iter()
                .map(format_type_ref)
                .collect::<Vec<_>>()
                .join(" | ")
        ),
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

pub(crate) fn text_size(offset: u32) -> TextSize {
    TextSize::from(offset)
}
