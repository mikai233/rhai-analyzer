use rhai_syntax::*;

use crate::ContainerLayoutStyle;
use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;
use crate::formatter::syntax::exprs::{
    DelimitedItemDoc, DelimitedNodeSpec, boundary_from_element_to_token,
    boundary_from_token_to_element, gap_requires_trivia_layout, hard_lines, merge_boundary_gaps,
    range_end, range_start,
};

impl Formatter<'_> {
    pub(crate) fn format_array_item_list_body_doc(
        &self,
        items: ArrayItemList,
        indent: usize,
    ) -> Doc {
        self.format_array_item_list_doc(items, indent)
    }

    pub(crate) fn format_interpolation_item_list_body_doc(
        &self,
        items: InterpolationItemList,
        indent: usize,
    ) -> Doc {
        self.format_interpolation_item_docs(items.items().collect::<Vec<_>>(), indent)
    }

    pub(crate) fn format_string_part_list_body_doc(
        &self,
        parts: StringPartList,
        indent: usize,
    ) -> Doc {
        self.format_string_part_docs(parts.parts().collect::<Vec<_>>(), indent)
    }

    pub(crate) fn format_object_field_list_body_doc(
        &self,
        fields: ObjectFieldList,
        indent: usize,
    ) -> Doc {
        let item_docs = self.object_field_list_items(fields.clone(), indent);
        self.format_comma_separated_body_doc(&fields.syntax(), item_docs)
    }

    pub(crate) fn format_array_doc(&self, array: ArrayExpr, indent: usize) -> Doc {
        array
            .items()
            .map(|items| self.format_array_item_list_doc(items, indent))
            .unwrap_or_else(|| Doc::text("[]"))
    }

    pub(crate) fn format_object_doc(&self, object: ObjectExpr, indent: usize) -> Doc {
        let fields = object
            .field_list()
            .map(|fields| self.object_field_list_items(fields, indent + 1))
            .unwrap_or_default();
        self.format_delimited_node_doc(
            DelimitedNodeSpec {
                node: &object.syntax(),
                open_kind: TokenKind::HashBraceOpen,
                close_kind: TokenKind::CloseBrace,
                open: "#{",
                close: "}",
            },
            fields,
            Some(60),
        )
    }

    pub(crate) fn object_field_list_items(
        &self,
        fields: ObjectFieldList,
        indent: usize,
    ) -> Vec<DelimitedItemDoc> {
        fields
            .fields()
            .map(|field| DelimitedItemDoc {
                element: field.syntax().into(),
                range: field.syntax().text_range(),
                doc: self.format_object_field_doc(field, indent),
            })
            .collect()
    }

    pub(crate) fn format_object_field_doc(&self, field: ObjectField, indent: usize) -> Doc {
        if self.object_field_requires_raw_fallback(field.clone()) {
            return Doc::text(self.raw(field.syntax()));
        }

        let name_token = field.name_token();
        let name = field
            .name_token()
            .map(|token| token.text().to_owned())
            .unwrap_or_default();
        let value_expr = field.value();
        let value = value_expr
            .clone()
            .map(|value| self.format_expr_doc(value, indent))
            .unwrap_or_else(|| Doc::text(""));
        let Some(name_token) = name_token else {
            return Doc::concat(vec![Doc::text(format!("{name}: ")), value]);
        };
        let Some(colon_token) = field
            .syntax()
            .direct_significant_tokens()
            .find(|token| token.kind().token_kind() == Some(TokenKind::Colon))
        else {
            return Doc::concat(vec![Doc::text(format!("{name}: ")), value]);
        };
        let Some(value_expr) = value_expr else {
            return Doc::concat(vec![Doc::text(format!("{name}: ")), value]);
        };

        let before_colon_gap = self
            .boundary_trivia(
                field.syntax(),
                TriviaBoundary::TokenToken(name_token.clone(), colon_token.clone()),
            )
            .unwrap_or_default();
        let after_colon_gap = self
            .boundary_trivia(
                field.syntax(),
                TriviaBoundary::TokenNode(colon_token.clone(), value_expr.syntax()),
            )
            .unwrap_or_default();

        if before_colon_gap.has_comments() || after_colon_gap.has_comments() {
            return Doc::concat(vec![
                Doc::text(name),
                self.tight_comment_gap_from_gap(&before_colon_gap, false),
                Doc::text(":"),
                self.space_or_tight_gap_from_gap(&after_colon_gap),
                value,
            ]);
        }

        Doc::concat(vec![Doc::text(format!("{name}: ")), value])
    }

    pub(crate) fn format_interpolated_string_doc(
        &self,
        string: InterpolatedStringExpr,
        indent: usize,
    ) -> Doc {
        let mut parts = vec![Doc::text("`")];
        parts.push(
            string
                .part_list()
                .map(|part_list| self.format_string_part_list_body_doc(part_list, indent))
                .unwrap_or_else(Doc::nil),
        );

        parts.push(Doc::text("`"));
        Doc::concat(parts)
    }

    pub(crate) fn format_interpolation_body_doc(
        &self,
        body: rhai_syntax::InterpolationBody,
        indent: usize,
    ) -> Doc {
        if self.node_has_unowned_comments(body.syntax()) {
            return Doc::text(self.raw(body.syntax()));
        }

        body.item_list()
            .map(|items| self.format_interpolation_item_list_body_doc(items, indent))
            .unwrap_or_else(Doc::nil)
    }

    pub(crate) fn format_interpolation_item_docs(
        &self,
        items: Vec<rhai_syntax::Item>,
        indent: usize,
    ) -> Doc {
        if items.is_empty() {
            return Doc::nil();
        }

        let item_docs = items
            .iter()
            .map(|item| self.format_item(item.clone(), indent))
            .collect::<Vec<_>>();
        let inline_fragments = item_docs
            .iter()
            .map(|item| self.render_fragment(item, 0))
            .collect::<Vec<_>>();
        let inline = inline_fragments.join(" ");
        let should_inline = inline_fragments.iter().all(|item| !item.contains('\n'))
            && inline.chars().count() <= self.options.max_line_length.saturating_sub(3);
        if should_inline {
            return Doc::text(inline);
        }

        let mut parts = vec![Doc::hard_line()];
        for (index, item) in item_docs.into_iter().enumerate() {
            if index > 0 {
                parts.push(Doc::hard_line());
            }
            parts.push(item);
        }
        parts.push(Doc::hard_line());

        Doc::indent(1, Doc::concat(parts))
    }

    pub(crate) fn format_string_part_docs(&self, parts: Vec<StringPart>, indent: usize) -> Doc {
        let mut docs = Vec::new();

        for part in parts {
            match part {
                StringPart::Segment(segment) => {
                    if let Some(token) = segment.text_token() {
                        docs.push(Doc::text(token.text()));
                    }
                }
                StringPart::Interpolation(interpolation) => {
                    let body = interpolation
                        .body()
                        .map(|body| self.format_interpolation_body_doc(body, indent))
                        .unwrap_or_else(Doc::nil);
                    docs.push(Doc::concat(vec![Doc::text("${"), body, Doc::text("}")]));
                }
            }
        }

        Doc::concat(docs)
    }

    pub(crate) fn format_delimited_doc_with_limit(
        &self,
        open: &str,
        close: &str,
        items: Vec<Doc>,
        inline_limit: Option<usize>,
    ) -> Doc {
        if items.is_empty() {
            return Doc::text(format!("{open}{close}"));
        }

        let inline_items = items
            .iter()
            .map(|item| self.render_fragment(item, 0))
            .collect::<Vec<_>>();
        let inline = format!("{open}{}{close}", inline_items.join(", "));
        let max_inline_width = match self.options.container_layout {
            ContainerLayoutStyle::Auto | ContainerLayoutStyle::PreferMultiLine => {
                inline_limit.unwrap_or(self.options.max_line_length)
            }
            ContainerLayoutStyle::PreferSingleLine => self.options.max_line_length,
        };
        let should_inline =
            self.should_inline_delimited_items(&inline_items, &inline, max_inline_width);
        if should_inline {
            return Doc::text(inline);
        }

        let mut item_docs = Vec::new();
        for (index, item) in items.into_iter().enumerate() {
            if index > 0 {
                item_docs.push(Doc::text(","));
                item_docs.push(Doc::soft_line());
            }
            item_docs.push(item);
        }

        let mut parts = vec![
            Doc::text(open),
            Doc::indent(
                1,
                Doc::concat(vec![Doc::soft_line(), Doc::concat(item_docs)]),
            ),
        ];
        if self.options.trailing_commas {
            parts.push(Doc::text(","));
        }
        parts.push(Doc::soft_line());
        parts.push(Doc::text(close));

        if inline_limit.is_some()
            || matches!(
                self.options.container_layout,
                ContainerLayoutStyle::PreferMultiLine
            )
        {
            Doc::concat(parts)
        } else {
            Doc::group(Doc::concat(parts))
        }
    }

    pub(crate) fn should_inline_delimited_items(
        &self,
        inline_items: &[String],
        inline: &str,
        max_inline_width: usize,
    ) -> bool {
        if matches!(
            self.options.container_layout,
            ContainerLayoutStyle::PreferMultiLine
        ) {
            return false;
        }

        inline_items.iter().all(|item| !item.contains('\n'))
            && inline.chars().count() <= max_inline_width
    }

    pub(crate) fn format_params_doc(&self, params: Option<ParamList>, _indent: usize) -> Doc {
        let names = params
            .clone()
            .map(|params| {
                params
                    .params()
                    .map(|param| DelimitedItemDoc {
                        element: param.clone().into(),
                        range: param.text_range(),
                        doc: Doc::text(param.text()),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        match params {
            Some(params) => self.format_delimited_node_doc(
                DelimitedNodeSpec {
                    node: &params.syntax(),
                    open_kind: TokenKind::OpenParen,
                    close_kind: TokenKind::CloseParen,
                    open: "(",
                    close: ")",
                },
                names,
                None,
            ),
            None => Doc::text("()"),
        }
    }

    pub(crate) fn arg_list_items(&self, args: ArgList, indent: usize) -> Vec<DelimitedItemDoc> {
        args.args()
            .map(|expr| DelimitedItemDoc {
                element: expr.syntax().into(),
                range: expr.syntax().text_range(),
                doc: self.format_expr_doc(expr, indent),
            })
            .collect::<Vec<_>>()
    }

    pub(crate) fn array_item_list_items(
        &self,
        items: ArrayItemList,
        indent: usize,
    ) -> Vec<DelimitedItemDoc> {
        items
            .exprs()
            .map(|expr| DelimitedItemDoc {
                element: expr.syntax().into(),
                range: expr.syntax().text_range(),
                doc: self.format_expr_doc(expr, indent + 1),
            })
            .collect::<Vec<_>>()
    }

    pub(crate) fn format_array_item_list_doc(&self, items: ArrayItemList, indent: usize) -> Doc {
        let item_docs = self.array_item_list_items(items.clone(), indent);
        self.format_delimited_node_doc(
            DelimitedNodeSpec {
                node: &items.syntax(),
                open_kind: TokenKind::OpenBracket,
                close_kind: TokenKind::CloseBracket,
                open: "[",
                close: "]",
            },
            item_docs,
            None,
        )
    }

    pub(crate) fn format_delimited_node_doc(
        &self,
        spec: DelimitedNodeSpec,
        items: Vec<DelimitedItemDoc>,
        inline_limit: Option<usize>,
    ) -> Doc {
        let open_end = self
            .token_range(spec.node.clone(), spec.open_kind)
            .map(range_end)
            .unwrap_or_else(|| range_start(spec.node.text_range()));
        let close_start = self
            .token_range(spec.node.clone(), spec.close_kind)
            .map(range_start)
            .unwrap_or_else(|| range_end(spec.node.text_range()));
        let item_elements = items
            .iter()
            .map(|item| item.element.clone())
            .collect::<Vec<_>>();
        let owned = self.owned_sequence_trivia(open_end, close_start, &item_elements);

        if items.is_empty() {
            if !gap_requires_trivia_layout(&owned.leading) {
                return Doc::text(format!("{}{}", spec.open, spec.close));
            }

            return Doc::concat(vec![
                Doc::text(spec.open),
                Doc::indent(1, self.leading_delimited_gap_doc(&owned.leading, false)),
                Doc::hard_line(),
                Doc::text(spec.close),
            ]);
        }

        let leading_gap = owned.leading.clone();
        let mut requires_trivia_layout = gap_requires_trivia_layout(&leading_gap);
        let commas = self
            .tokens(spec.node.clone(), TokenKind::Comma)
            .collect::<Vec<_>>();
        let direct_boundary_layout = commas.len() + 1 == items.len();

        if direct_boundary_layout {
            requires_trivia_layout |=
                self.comma_sequence_requires_trivia_layout(spec.node.clone(), &items, &commas);
        } else {
            for gap in &owned.between {
                requires_trivia_layout |= gap_requires_trivia_layout(gap);
            }
        }

        let trailing_gap = owned.trailing.clone();
        requires_trivia_layout |= gap_requires_trivia_layout(&trailing_gap);

        if !requires_trivia_layout {
            let docs = items.into_iter().map(|item| item.doc).collect::<Vec<_>>();
            return self.format_delimited_doc_with_limit(spec.open, spec.close, docs, inline_limit);
        }

        let mut body_parts = vec![self.leading_delimited_gap_doc(&leading_gap, true)];
        body_parts.extend(self.comma_sequence_body_parts(
            spec.node.clone(),
            items,
            commas,
            direct_boundary_layout,
        ));

        if self.options.trailing_commas {
            body_parts.push(Doc::text(","));
        }

        self.append_sequence_trailing_doc(&mut body_parts, &trailing_gap, true, 1);

        Doc::concat(vec![
            Doc::text(spec.open),
            Doc::indent(1, Doc::concat(body_parts)),
            Doc::hard_line(),
            Doc::text(spec.close),
        ])
    }

    pub(crate) fn format_comma_separated_body_doc(
        &self,
        node: &SyntaxNode,
        items: Vec<DelimitedItemDoc>,
    ) -> Doc {
        if items.is_empty() {
            return Doc::nil();
        }

        let item_elements = items
            .iter()
            .map(|item| item.element.clone())
            .collect::<Vec<_>>();
        let owned = self.owned_sequence_trivia(
            range_start(node.text_range()),
            range_end(node.text_range()),
            &item_elements,
        );
        let mut requires_trivia_layout = gap_requires_trivia_layout(&owned.trailing);
        let commas = self
            .tokens(node.clone(), TokenKind::Comma)
            .collect::<Vec<_>>();
        let direct_boundary_layout = commas.len() + 1 == items.len();
        if direct_boundary_layout {
            requires_trivia_layout |=
                self.comma_sequence_requires_trivia_layout(node.clone(), &items, &commas);
        } else {
            for gap in &owned.between {
                requires_trivia_layout |= gap_requires_trivia_layout(gap);
            }
        }
        let trailing_gap = owned.trailing.clone();
        requires_trivia_layout |= gap_requires_trivia_layout(&trailing_gap);

        let inline_items = items
            .iter()
            .map(|item| self.render_fragment(&item.doc, 0))
            .collect::<Vec<_>>();
        let inline = inline_items.join(", ");
        let should_inline = !requires_trivia_layout
            && self.should_inline_delimited_items(
                &inline_items,
                &inline,
                self.options.max_line_length,
            );
        if should_inline {
            return Doc::text(inline);
        }

        let mut body_parts = vec![Doc::hard_line()];
        body_parts.extend(self.comma_sequence_body_parts(
            node.clone(),
            items,
            commas,
            direct_boundary_layout,
        ));

        if self.options.trailing_commas {
            body_parts.push(Doc::text(","));
        }
        self.append_sequence_trailing_doc(&mut body_parts, &trailing_gap, true, 1);
        body_parts.push(Doc::hard_line());

        Doc::indent(1, Doc::concat(body_parts))
    }

    pub(crate) fn leading_delimited_gap_doc(
        &self,
        gap: &GapTrivia,
        include_terminal_newline: bool,
    ) -> Doc {
        if gap.has_vertical_comments() {
            let vertical_comments = gap.vertical_comments();
            let mut parts = vec![hard_lines(vertical_comments[0].blank_lines_before + 1)];
            parts.push(self.render_line_comments_doc(vertical_comments));

            let suffix_newlines =
                gap.trailing_blank_lines_before_next + usize::from(include_terminal_newline);
            if suffix_newlines > 0 {
                parts.push(hard_lines(suffix_newlines));
            }

            return Doc::concat(parts);
        }

        hard_lines(gap.trailing_blank_lines_before_next + usize::from(include_terminal_newline))
    }

    pub(crate) fn comma_sequence_requires_trivia_layout(
        &self,
        node: SyntaxNode,
        items: &[DelimitedItemDoc],
        commas: &[SyntaxToken],
    ) -> bool {
        let mut previous_item = &items[0];
        for (comma_token, item) in commas.iter().zip(items.iter().skip(1)) {
            let before_gap = self
                .boundary_trivia(
                    node.clone(),
                    boundary_from_element_to_token(&previous_item.element, comma_token.clone()),
                )
                .unwrap_or_default();
            let after_gap = self
                .boundary_trivia(
                    node.clone(),
                    boundary_from_token_to_element(comma_token.clone(), &item.element),
                )
                .unwrap_or_default();
            if gap_requires_trivia_layout(&before_gap) || gap_requires_trivia_layout(&after_gap) {
                return true;
            }
            previous_item = item;
        }
        false
    }

    pub(crate) fn comma_sequence_body_parts(
        &self,
        node: SyntaxNode,
        items: Vec<DelimitedItemDoc>,
        commas: Vec<SyntaxToken>,
        direct_boundary_layout: bool,
    ) -> Vec<Doc> {
        let mut body_parts = Vec::new();
        let mut items = items.into_iter();
        let first_item = items.next().expect("non-empty comma-separated items");
        let mut previous_element = first_item.element.clone();
        body_parts.push(first_item.doc);

        if direct_boundary_layout {
            for (comma_token, item) in commas.into_iter().zip(items) {
                let before_gap = self
                    .boundary_trivia(
                        node.clone(),
                        boundary_from_element_to_token(&previous_element, comma_token.clone()),
                    )
                    .unwrap_or_default();
                let after_gap = self
                    .boundary_trivia(
                        node.clone(),
                        boundary_from_token_to_element(comma_token.clone(), &item.element),
                    )
                    .unwrap_or_default();
                body_parts.push(Doc::text(","));
                body_parts.push(self.gap_separator_doc(
                    &merge_boundary_gaps(before_gap, after_gap),
                    1,
                    true,
                    true,
                ));
                body_parts.push(item.doc);
                previous_element = item.element;
            }
        } else {
            let mut previous_end = range_end(first_item.range);
            for item in items {
                let gap = self.comment_gap(previous_end, range_start(item.range), true, true);
                body_parts.push(Doc::text(","));
                body_parts.push(self.gap_separator_doc(&gap, 1, true, true));
                body_parts.push(item.doc);
                previous_end = range_end(item.range);
            }
        }

        body_parts
    }
}
