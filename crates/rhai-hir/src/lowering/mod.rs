mod calls;
mod ctx;
mod exprs;
mod flow;
mod items;
mod modules;
mod resolve;

use rhai_syntax::{AstNode, Parse, Root};

use crate::LoweredFile;
use crate::model::ScopeKind;

pub fn lower_file(parse: &Parse) -> LoweredFile {
    let root = Root::cast(parse.root()).expect("root syntax node should cast");
    let mut ctx = crate::lowering::ctx::LoweringContext::new(parse);
    let file_scope = ctx.new_scope(ScopeKind::File, parse.root().text_range(), None);

    if let Some(items) = root.item_list() {
        for item in items.items() {
            ctx.lower_item(item, file_scope);
        }
    }

    ctx.finish()
}
