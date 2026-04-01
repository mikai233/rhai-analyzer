use crate::{SemanticToken, SemanticTokenKind, SemanticTokenModifier};
use rhai_db::DatabaseSnapshot;
use rhai_hir::{FileHir, ReferenceKind, SymbolKind};
use rhai_syntax::{SyntaxNodeExt, SyntaxToken, TextSize, TokenKind};
use rhai_vfs::FileId;

struct TokenContext<'a> {
    snapshot: &'a DatabaseSnapshot,
    file_id: FileId,
    hir: Option<&'a FileHir>,
}

pub(crate) fn semantic_tokens(snapshot: &DatabaseSnapshot, file_id: FileId) -> Vec<SemanticToken> {
    let Some(parse) = snapshot.parse(file_id) else {
        return Vec::new();
    };
    let root = parse.root();
    let tokens: Vec<_> = root
        .raw_tokens()
        .filter(|token| token.kind().token_kind().is_some())
        .collect();
    let hir = snapshot.hir(file_id);
    let context = TokenContext {
        snapshot,
        file_id,
        hir: hir.as_deref(),
    };

    tokens
        .iter()
        .filter_map(|token| semantic_token(&context, token))
        .collect()
}

fn semantic_token(context: &TokenContext<'_>, token: &SyntaxToken) -> Option<SemanticToken> {
    let kind = semantic_token_kind(context, token)?;

    Some(SemanticToken {
        range: token.text_range(),
        kind,
        modifiers: semantic_token_modifiers(context, token),
    })
}

fn semantic_token_kind(
    context: &TokenContext<'_>,
    token: &SyntaxToken,
) -> Option<SemanticTokenKind> {
    match token.kind().token_kind()? {
        TokenKind::LineComment
        | TokenKind::DocLineComment
        | TokenKind::BlockComment
        | TokenKind::DocBlockComment
        | TokenKind::Shebang => Some(SemanticTokenKind::Comment),
        TokenKind::String | TokenKind::RawString | TokenKind::BacktickString | TokenKind::Char => {
            if is_typed_method_receiver(token) {
                Some(SemanticTokenKind::Type)
            } else {
                Some(SemanticTokenKind::String)
            }
        }
        TokenKind::Int | TokenKind::Float => Some(SemanticTokenKind::Number),
        TokenKind::Ident => classify_identifier_token(context, token),
        TokenKind::ThisKw => Some(SemanticTokenKind::Variable),
        kind if is_keyword(kind) => Some(SemanticTokenKind::Keyword),
        kind if is_operator(kind) => Some(SemanticTokenKind::Operator),
        _ => None,
    }
}

fn semantic_token_modifiers(
    context: &TokenContext<'_>,
    token: &SyntaxToken,
) -> Vec<SemanticTokenModifier> {
    let kind = semantic_token_kind(context, token);
    if is_typed_method_receiver(token) {
        let mut modifiers = vec![SemanticTokenModifier::Declaration];
        if host_type_named(
            context.snapshot,
            typed_method_receiver_name(token).as_deref(),
        ) {
            modifiers.push(SemanticTokenModifier::DefaultLibrary);
        }
        return modifiers;
    }

    let Some(hir) = context.hir else {
        return Vec::new();
    };
    let mut modifiers = Vec::new();

    if let Some(symbol) = hir.symbol_at(token.text_range()) {
        modifiers.push(SemanticTokenModifier::Declaration);

        if matches!(hir.symbol(symbol).kind, SymbolKind::Constant) {
            modifiers.push(SemanticTokenModifier::Readonly);
        }
    } else if let Some(reference) = hir.reference_at(token.text_range())
        && let Some(target) = hir.reference(reference).target
        && matches!(hir.symbol(target).kind, SymbolKind::Constant)
    {
        modifiers.push(SemanticTokenModifier::Readonly);
    }

    if is_default_library_token(context, hir, token, kind) {
        modifiers.push(SemanticTokenModifier::DefaultLibrary);
    }

    modifiers
}

fn classify_identifier_token(
    context: &TokenContext<'_>,
    token: &SyntaxToken,
) -> Option<SemanticTokenKind> {
    if is_typed_method_receiver(token) {
        return Some(SemanticTokenKind::Type);
    }

    let hir = context.hir?;

    if let Some(symbol) = hir.symbol_at(token.text_range()) {
        return Some(symbol_kind_semantic_token_kind(hir.symbol(symbol).kind));
    }

    if let Some(reference_id) = hir.reference_at(token.text_range()) {
        let reference = hir.reference(reference_id);

        if let Some(target) = reference.target
            && reference.kind != ReferenceKind::PathSegment
        {
            return Some(symbol_kind_semantic_token_kind(hir.symbol(target).kind));
        }

        return Some(match reference.kind {
            ReferenceKind::Field => {
                if is_method_callee_token(token) {
                    SemanticTokenKind::Method
                } else {
                    SemanticTokenKind::Property
                }
            }
            ReferenceKind::PathSegment => {
                if let Some(kind) = external_signature_kind_for_path(
                    context.snapshot,
                    hir,
                    token.text_range().start(),
                    token,
                ) {
                    kind
                } else if is_final_path_segment(token)
                    && let Some(kind) = context
                        .snapshot
                        .goto_definition(context.file_id, token.text_range().start())
                        .first()
                        .map(|target| target.target.kind)
                {
                    symbol_kind_semantic_token_kind(kind)
                } else {
                    SemanticTokenKind::Namespace
                }
            }
            ReferenceKind::Name | ReferenceKind::This => {
                if matches!(
                    next_significant_token(token).as_ref().and_then(token_kind),
                    Some(TokenKind::ColonColon)
                ) {
                    SemanticTokenKind::Namespace
                } else if is_call_like_token(token)
                    && (context
                        .snapshot
                        .global_function(reference.name.as_str())
                        .is_some()
                        || context
                            .snapshot
                            .external_signatures()
                            .get(reference.name.as_str())
                            .is_some())
                {
                    SemanticTokenKind::Function
                } else {
                    SemanticTokenKind::Variable
                }
            }
        });
    }

    let text = token.text();
    if is_call_like_token(token)
        && (context.snapshot.global_function(text).is_some()
            || context.snapshot.external_signatures().get(text).is_some())
    {
        return Some(SemanticTokenKind::Function);
    }

    if context
        .snapshot
        .host_types()
        .iter()
        .any(|host_type| host_type.name == text)
    {
        return Some(SemanticTokenKind::Type);
    }

    None
}

fn symbol_kind_semantic_token_kind(kind: SymbolKind) -> SemanticTokenKind {
    match kind {
        SymbolKind::Function => SemanticTokenKind::Function,
        SymbolKind::Parameter => SemanticTokenKind::Parameter,
        SymbolKind::ImportAlias | SymbolKind::ExportAlias => SemanticTokenKind::Namespace,
        SymbolKind::Variable | SymbolKind::Constant => SemanticTokenKind::Variable,
    }
}

fn is_typed_method_receiver(token: &SyntaxToken) -> bool {
    if !matches!(
        token_kind(token),
        Some(TokenKind::Ident | TokenKind::String)
    ) {
        return false;
    }

    let previous = previous_significant_token(token);
    let next = next_significant_token(token);

    matches!(
        (
            previous.as_ref().and_then(token_kind),
            next.as_ref().and_then(token_kind)
        ),
        (Some(TokenKind::FnKw), Some(TokenKind::Dot))
    )
}

fn is_method_callee_token(token: &SyntaxToken) -> bool {
    matches!(
        next_significant_token(token).as_ref().and_then(token_kind),
        Some(TokenKind::OpenParen)
    )
}

fn is_final_path_segment(token: &SyntaxToken) -> bool {
    !matches!(
        next_significant_token(token).as_ref().and_then(token_kind),
        Some(TokenKind::ColonColon)
    )
}

fn previous_significant_token(token: &SyntaxToken) -> Option<SyntaxToken> {
    let mut cursor = token.prev_token();
    while let Some(current) = cursor {
        if token_kind(&current).is_some_and(|kind| !kind.is_trivia()) {
            return Some(current);
        }
        cursor = current.prev_token();
    }
    None
}

fn next_significant_token(token: &SyntaxToken) -> Option<SyntaxToken> {
    let mut cursor = token.next_token();
    while let Some(current) = cursor {
        if token_kind(&current).is_some_and(|kind| !kind.is_trivia()) {
            return Some(current);
        }
        cursor = current.next_token();
    }
    None
}

fn is_keyword(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::LetKw
            | TokenKind::ConstKw
            | TokenKind::IfKw
            | TokenKind::ElseKw
            | TokenKind::SwitchKw
            | TokenKind::DoKw
            | TokenKind::WhileKw
            | TokenKind::UntilKw
            | TokenKind::LoopKw
            | TokenKind::ForKw
            | TokenKind::InKw
            | TokenKind::ContinueKw
            | TokenKind::BreakKw
            | TokenKind::ReturnKw
            | TokenKind::ThrowKw
            | TokenKind::TryKw
            | TokenKind::CatchKw
            | TokenKind::ImportKw
            | TokenKind::ExportKw
            | TokenKind::AsKw
            | TokenKind::GlobalKw
            | TokenKind::PrivateKw
            | TokenKind::FnKw
            | TokenKind::TrueKw
            | TokenKind::FalseKw
            | TokenKind::FnPtrKw
            | TokenKind::CallKw
            | TokenKind::CurryKw
            | TokenKind::IsSharedKw
            | TokenKind::IsDefFnKw
            | TokenKind::IsDefVarKw
            | TokenKind::TypeOfKw
            | TokenKind::PrintKw
            | TokenKind::DebugKw
            | TokenKind::EvalKw
    )
}

fn is_operator(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::ColonColon
            | TokenKind::Dot
            | TokenKind::QuestionDot
            | TokenKind::QuestionOpenBracket
            | TokenKind::FatArrow
            | TokenKind::Eq
            | TokenKind::PlusEq
            | TokenKind::MinusEq
            | TokenKind::StarEq
            | TokenKind::SlashEq
            | TokenKind::PercentEq
            | TokenKind::StarStarEq
            | TokenKind::ShlEq
            | TokenKind::ShrEq
            | TokenKind::AmpEq
            | TokenKind::PipeEq
            | TokenKind::CaretEq
            | TokenKind::QuestionQuestionEq
            | TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::StarStar
            | TokenKind::Shl
            | TokenKind::Shr
            | TokenKind::Amp
            | TokenKind::Pipe
            | TokenKind::Caret
            | TokenKind::AmpAmp
            | TokenKind::PipePipe
            | TokenKind::Bang
            | TokenKind::EqEq
            | TokenKind::BangEq
            | TokenKind::Gt
            | TokenKind::GtEq
            | TokenKind::Lt
            | TokenKind::LtEq
            | TokenKind::QuestionQuestion
            | TokenKind::Range
            | TokenKind::RangeEq
    )
}

fn is_default_library_token(
    context: &TokenContext<'_>,
    hir: &FileHir,
    token: &SyntaxToken,
    kind: Option<SemanticTokenKind>,
) -> bool {
    let text = token.text();

    if matches!(kind, Some(SemanticTokenKind::Type)) {
        return host_type_named(context.snapshot, Some(unquote_type_name(text)));
    }

    if matches!(kind, Some(SemanticTokenKind::Function))
        && is_call_like_token(token)
        && (context.snapshot.global_function(text).is_some()
            || context.snapshot.external_signatures().get(text).is_some())
    {
        return true;
    }

    if let Some(reference_id) = hir.reference_at(token.text_range()) {
        let reference = hir.reference(reference_id);

        if matches!(reference.kind, ReferenceKind::Field)
            && matches!(
                kind,
                Some(SemanticTokenKind::Method | SemanticTokenKind::Property)
            )
            && member_resolves_to_host_method(context.snapshot, context.file_id, hir, reference_id)
        {
            return true;
        }

        if reference.kind == ReferenceKind::PathSegment
            || (reference.kind == ReferenceKind::Name
                && matches!(
                    next_significant_token(token).as_ref().and_then(token_kind),
                    Some(TokenKind::ColonColon)
                ))
        {
            if is_path_external_signature(context.snapshot, hir, token.text_range().start(), token)
            {
                return true;
            }

            if namespace_has_external_prefix(context.snapshot, text) {
                return true;
            }
        }
    }

    false
}

fn host_type_named(snapshot: &DatabaseSnapshot, name: Option<&str>) -> bool {
    let Some(name) = name else {
        return false;
    };

    snapshot
        .host_types()
        .iter()
        .any(|host_type| host_type.name == name)
}

fn typed_method_receiver_name(token: &SyntaxToken) -> Option<String> {
    match token_kind(token)? {
        TokenKind::Ident => Some(token.text().to_owned()),
        TokenKind::String => Some(unquote_type_name(token.text()).to_owned()),
        _ => None,
    }
}

fn unquote_type_name(text: &str) -> &str {
    text.strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
        .unwrap_or(text)
}

fn is_call_like_token(token: &SyntaxToken) -> bool {
    matches!(
        next_significant_token(token).as_ref().and_then(token_kind),
        Some(TokenKind::OpenParen | TokenKind::Bang)
    )
}

fn member_resolves_to_host_method(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
    reference_id: rhai_hir::ReferenceId,
) -> bool {
    let Some(access) = hir
        .member_accesses
        .iter()
        .find(|access| access.field_reference == reference_id)
    else {
        return false;
    };
    let Some(receiver_ty) =
        snapshot.inferred_expr_type_at(file_id, hir.expr(access.receiver).range.start())
    else {
        return false;
    };
    let method_name = hir.reference(reference_id).name.as_str();

    host_type_matches_method(snapshot, receiver_ty, method_name)
}

fn host_type_matches_method(
    snapshot: &DatabaseSnapshot,
    receiver_ty: &rhai_hir::TypeRef,
    method_name: &str,
) -> bool {
    match receiver_ty {
        rhai_hir::TypeRef::Union(items) | rhai_hir::TypeRef::Ambiguous(items) => items
            .iter()
            .any(|item| host_type_matches_method(snapshot, item, method_name)),
        rhai_hir::TypeRef::Nullable(inner) => {
            host_type_matches_method(snapshot, inner, method_name)
        }
        rhai_hir::TypeRef::Named(name) | rhai_hir::TypeRef::Applied { name, .. } => snapshot
            .host_types()
            .iter()
            .find(|host_type| host_type.name == *name)
            .is_some_and(|host_type| {
                host_type
                    .methods
                    .iter()
                    .any(|method| method.name == method_name)
            }),
        rhai_hir::TypeRef::Int => {
            host_type_named(snapshot, Some("int"))
                && host_type_matches_method(
                    snapshot,
                    &rhai_hir::TypeRef::Named("int".to_owned()),
                    method_name,
                )
        }
        rhai_hir::TypeRef::Float => {
            host_type_named(snapshot, Some("float"))
                && host_type_matches_method(
                    snapshot,
                    &rhai_hir::TypeRef::Named("float".to_owned()),
                    method_name,
                )
        }
        rhai_hir::TypeRef::Char => {
            host_type_named(snapshot, Some("char"))
                && host_type_matches_method(
                    snapshot,
                    &rhai_hir::TypeRef::Named("char".to_owned()),
                    method_name,
                )
        }
        rhai_hir::TypeRef::String => {
            host_type_named(snapshot, Some("string"))
                && host_type_matches_method(
                    snapshot,
                    &rhai_hir::TypeRef::Named("string".to_owned()),
                    method_name,
                )
        }
        rhai_hir::TypeRef::Array(_) => {
            host_type_named(snapshot, Some("array"))
                && host_type_matches_method(
                    snapshot,
                    &rhai_hir::TypeRef::Named("array".to_owned()),
                    method_name,
                )
        }
        rhai_hir::TypeRef::Map(_, _) | rhai_hir::TypeRef::Object(_) => {
            host_type_named(snapshot, Some("map"))
                && host_type_matches_method(
                    snapshot,
                    &rhai_hir::TypeRef::Named("map".to_owned()),
                    method_name,
                )
        }
        rhai_hir::TypeRef::Blob => {
            host_type_named(snapshot, Some("blob"))
                && host_type_matches_method(
                    snapshot,
                    &rhai_hir::TypeRef::Named("blob".to_owned()),
                    method_name,
                )
        }
        rhai_hir::TypeRef::Timestamp => {
            host_type_named(snapshot, Some("timestamp"))
                && host_type_matches_method(
                    snapshot,
                    &rhai_hir::TypeRef::Named("timestamp".to_owned()),
                    method_name,
                )
        }
        rhai_hir::TypeRef::Range => {
            host_type_named(snapshot, Some("range"))
                && host_type_matches_method(
                    snapshot,
                    &rhai_hir::TypeRef::Named("range".to_owned()),
                    method_name,
                )
        }
        rhai_hir::TypeRef::RangeInclusive => {
            host_type_named(snapshot, Some("range="))
                && host_type_matches_method(
                    snapshot,
                    &rhai_hir::TypeRef::Named("range=".to_owned()),
                    method_name,
                )
        }
        _ => false,
    }
}

fn is_path_external_signature(
    snapshot: &DatabaseSnapshot,
    hir: &FileHir,
    offset: TextSize,
    token: &SyntaxToken,
) -> bool {
    external_signature_kind_for_path(snapshot, hir, offset, token).is_some()
}

fn external_signature_kind_for_path(
    snapshot: &DatabaseSnapshot,
    hir: &FileHir,
    offset: TextSize,
    token: &SyntaxToken,
) -> Option<SemanticTokenKind> {
    let expr = hir.expr_at_offset(offset)?;
    let qualified_name = hir.qualified_path_name(expr)?;
    let ty = snapshot.external_signatures().get(&qualified_name)?;

    if is_call_like_token(token) {
        return Some(if matches!(ty, rhai_hir::TypeRef::Function(_)) {
            SemanticTokenKind::Function
        } else {
            SemanticTokenKind::Variable
        });
    }

    Some(match ty {
        rhai_hir::TypeRef::Function(_) => SemanticTokenKind::Function,
        _ => SemanticTokenKind::Variable,
    })
}

fn namespace_has_external_prefix(snapshot: &DatabaseSnapshot, text: &str) -> bool {
    snapshot
        .external_signatures()
        .iter()
        .any(|(name, _)| name.starts_with(&format!("{text}::")))
}

fn token_kind(token: &SyntaxToken) -> Option<TokenKind> {
    token.kind().token_kind()
}
