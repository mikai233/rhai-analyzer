use crate::types::FileCommentDirectives;
use rhai_hir::parse_type_ref;
use rhai_syntax::{CommentDirectiveKind, Parse, collect_comment_directives};

pub(crate) fn file_comment_directives(parse: &Parse) -> FileCommentDirectives {
    let mut directives = FileCommentDirectives::default();

    for directive in collect_comment_directives(&parse.root(), parse.text()) {
        match directive.kind {
            CommentDirectiveKind::Extern { name, ty } => {
                if let Some((module, _)) = name.split_once("::") {
                    directives.external_modules.insert(module.to_owned());
                }
                directives.allowed_unresolved_names.insert(name.clone());
                if let Some(ty) = ty.as_deref().and_then(parse_type_ref) {
                    directives.external_signatures.insert(name, ty);
                }
            }
            CommentDirectiveKind::Module { name } => {
                directives.external_modules.insert(name);
            }
            CommentDirectiveKind::AllowUnresolved { name } => {
                directives.allowed_unresolved_names.insert(name);
            }
            CommentDirectiveKind::AllowUnresolvedImport { name } => {
                directives.allowed_unresolved_imports.insert(name);
            }
            CommentDirectiveKind::FormatSkip => {}
        }
    }

    directives
}
