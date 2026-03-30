use crate::{SemanticToken, SemanticTokenKind, SemanticTokenModifier};
use rhai_db::DatabaseSnapshot;
use rhai_hir::{FileHir, ReferenceKind, SymbolKind};
use rhai_syntax::{SyntaxToken, TextSize, TokenKind};
use rhai_vfs::FileId;

struct TokenContext<'a> {
    snapshot: &'a DatabaseSnapshot,
    file_id: FileId,
    hir: Option<&'a FileHir>,
    tokens: &'a [SyntaxToken],
    source: &'a str,
}

pub(crate) fn semantic_tokens(snapshot: &DatabaseSnapshot, file_id: FileId) -> Vec<SemanticToken> {
    let Some(parse) = snapshot.parse(file_id) else {
        return Vec::new();
    };
    let hir = snapshot.hir(file_id);
    let context = TokenContext {
        snapshot,
        file_id,
        hir: hir.as_deref(),
        tokens: parse.tokens(),
        source: parse.text(),
    };

    parse
        .tokens()
        .iter()
        .enumerate()
        .filter_map(|(index, token)| semantic_token(&context, index, *token))
        .collect()
}

fn semantic_token(
    context: &TokenContext<'_>,
    index: usize,
    token: SyntaxToken,
) -> Option<SemanticToken> {
    let kind = semantic_token_kind(context, index, token)?;

    Some(SemanticToken {
        range: token.range(),
        kind,
        modifiers: semantic_token_modifiers(context, index, token),
    })
}

fn semantic_token_kind(
    context: &TokenContext<'_>,
    index: usize,
    token: SyntaxToken,
) -> Option<SemanticTokenKind> {
    match token.kind() {
        TokenKind::LineComment
        | TokenKind::DocLineComment
        | TokenKind::BlockComment
        | TokenKind::DocBlockComment
        | TokenKind::Shebang => Some(SemanticTokenKind::Comment),
        TokenKind::String | TokenKind::RawString | TokenKind::BacktickString | TokenKind::Char => {
            if is_typed_method_receiver(context.tokens, index) {
                Some(SemanticTokenKind::Type)
            } else {
                Some(SemanticTokenKind::String)
            }
        }
        TokenKind::Int | TokenKind::Float => Some(SemanticTokenKind::Number),
        TokenKind::Ident => classify_identifier_token(context, index, token),
        TokenKind::ThisKw => Some(SemanticTokenKind::Variable),
        kind if is_keyword(kind) => Some(SemanticTokenKind::Keyword),
        kind if is_operator(kind) => Some(SemanticTokenKind::Operator),
        _ => None,
    }
}

fn semantic_token_modifiers(
    context: &TokenContext<'_>,
    index: usize,
    token: SyntaxToken,
) -> Vec<SemanticTokenModifier> {
    let kind = semantic_token_kind(context, index, token);
    if is_typed_method_receiver(context.tokens, index) {
        let mut modifiers = vec![SemanticTokenModifier::Declaration];
        if host_type_named(
            context.snapshot,
            typed_method_receiver_name(token, context.source).as_deref(),
        ) {
            modifiers.push(SemanticTokenModifier::DefaultLibrary);
        }
        return modifiers;
    }

    let Some(hir) = context.hir else {
        return Vec::new();
    };
    let mut modifiers = Vec::new();

    if let Some(symbol) = hir.symbol_at(token.range()) {
        modifiers.push(SemanticTokenModifier::Declaration);

        if matches!(hir.symbol(symbol).kind, SymbolKind::Constant) {
            modifiers.push(SemanticTokenModifier::Readonly);
        }
    } else if let Some(reference) = hir.reference_at(token.range())
        && let Some(target) = hir.reference(reference).target
        && matches!(hir.symbol(target).kind, SymbolKind::Constant)
    {
        modifiers.push(SemanticTokenModifier::Readonly);
    }

    if is_default_library_token(context, hir, index, token, kind) {
        modifiers.push(SemanticTokenModifier::DefaultLibrary);
    }

    modifiers
}

fn classify_identifier_token(
    context: &TokenContext<'_>,
    index: usize,
    token: SyntaxToken,
) -> Option<SemanticTokenKind> {
    if is_typed_method_receiver(context.tokens, index) {
        return Some(SemanticTokenKind::Type);
    }

    let hir = context.hir?;

    if let Some(symbol) = hir.symbol_at(token.range()) {
        return Some(symbol_kind_semantic_token_kind(hir.symbol(symbol).kind));
    }

    if let Some(reference_id) = hir.reference_at(token.range()) {
        let reference = hir.reference(reference_id);

        if let Some(target) = reference.target
            && reference.kind != ReferenceKind::PathSegment
        {
            return Some(symbol_kind_semantic_token_kind(hir.symbol(target).kind));
        }

        return Some(match reference.kind {
            ReferenceKind::Field => {
                if is_method_callee_token(context.tokens, index) {
                    SemanticTokenKind::Method
                } else {
                    SemanticTokenKind::Property
                }
            }
            ReferenceKind::PathSegment => {
                if let Some(kind) = external_signature_kind_for_path(
                    context.snapshot,
                    hir,
                    token.range().start(),
                    context.tokens,
                    index,
                ) {
                    kind
                } else if is_final_path_segment(context.tokens, index)
                    && let Some(kind) = context
                        .snapshot
                        .goto_definition(context.file_id, token.range().start())
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
                    next_significant_token(context.tokens, index).map(SyntaxToken::kind),
                    Some(TokenKind::ColonColon)
                ) {
                    SemanticTokenKind::Namespace
                } else if is_call_like_token(context.tokens, index)
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

    let text = token.text(context.source);
    if is_call_like_token(context.tokens, index)
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

fn is_typed_method_receiver(tokens: &[SyntaxToken], index: usize) -> bool {
    let token = tokens[index];
    if !matches!(token.kind(), TokenKind::Ident | TokenKind::String) {
        return false;
    }

    let previous = previous_significant_token(tokens, index);
    let next = next_significant_token(tokens, index);

    matches!(
        (previous.map(SyntaxToken::kind), next.map(SyntaxToken::kind)),
        (Some(TokenKind::FnKw), Some(TokenKind::Dot))
    )
}

fn is_method_callee_token(tokens: &[SyntaxToken], index: usize) -> bool {
    matches!(
        next_significant_token(tokens, index).map(SyntaxToken::kind),
        Some(TokenKind::OpenParen)
    )
}

fn is_final_path_segment(tokens: &[SyntaxToken], index: usize) -> bool {
    !matches!(
        next_significant_token(tokens, index).map(SyntaxToken::kind),
        Some(TokenKind::ColonColon)
    )
}

fn previous_significant_token(tokens: &[SyntaxToken], index: usize) -> Option<SyntaxToken> {
    tokens[..index]
        .iter()
        .rev()
        .copied()
        .find(|token| !token.kind().is_trivia())
}

fn next_significant_token(tokens: &[SyntaxToken], index: usize) -> Option<SyntaxToken> {
    tokens[index + 1..]
        .iter()
        .copied()
        .find(|token| !token.kind().is_trivia())
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
    index: usize,
    token: SyntaxToken,
    kind: Option<SemanticTokenKind>,
) -> bool {
    let text = token.text(context.source);

    if matches!(kind, Some(SemanticTokenKind::Type)) {
        return host_type_named(context.snapshot, Some(unquote_type_name(text)));
    }

    if matches!(kind, Some(SemanticTokenKind::Function))
        && is_call_like_token(context.tokens, index)
        && (context.snapshot.global_function(text).is_some()
            || context.snapshot.external_signatures().get(text).is_some())
    {
        return true;
    }

    if let Some(reference_id) = hir.reference_at(token.range()) {
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
                    next_significant_token(context.tokens, index).map(SyntaxToken::kind),
                    Some(TokenKind::ColonColon)
                ))
        {
            if is_path_external_signature(
                context.snapshot,
                hir,
                token.range().start(),
                context.tokens,
                index,
            ) {
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

fn typed_method_receiver_name(token: SyntaxToken, source: &str) -> Option<String> {
    match token.kind() {
        TokenKind::Ident => Some(token.text(source).to_owned()),
        TokenKind::String => Some(unquote_type_name(token.text(source)).to_owned()),
        _ => None,
    }
}

fn unquote_type_name(text: &str) -> &str {
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        &text[1..text.len() - 1]
    } else {
        text
    }
}

fn is_call_like_token(tokens: &[SyntaxToken], index: usize) -> bool {
    matches!(
        next_significant_token(tokens, index).map(SyntaxToken::kind),
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
    tokens: &[SyntaxToken],
    index: usize,
) -> bool {
    external_signature_kind_for_path(snapshot, hir, offset, tokens, index).is_some()
}

fn external_signature_kind_for_path(
    snapshot: &DatabaseSnapshot,
    hir: &FileHir,
    offset: TextSize,
    tokens: &[SyntaxToken],
    index: usize,
) -> Option<SemanticTokenKind> {
    let expr = hir.expr_at_offset(offset)?;
    let qualified_name = hir.qualified_path_name(expr)?;
    let ty = snapshot.external_signatures().get(&qualified_name)?;

    if is_call_like_token(tokens, index) {
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
