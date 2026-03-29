use crate::lowering::ctx::{LoweringContext, PendingMutationKind};
use crate::model::{
    FileHir, ReferenceKind, ScopeId, ScopeKind, Symbol, SymbolId, SymbolKind, SymbolMutation,
    SymbolMutationKind,
};
use crate::ty::TypeRef;
use rhai_syntax::TextSize;

impl<'a> LoweringContext<'a> {
    pub(crate) fn finish(mut self) -> FileHir {
        self.resolve_references();
        self.resolve_call_mappings();
        self.resolve_value_flows();
        self.resolve_mutations();
        self.annotate_symbol_relationships();
        self.file
    }

    pub(crate) fn resolve_references(&mut self) {
        for symbol in &mut self.file.symbols {
            symbol.references.clear();
        }

        for index in 0..self.file.references.len() {
            let (kind, scope, range, name) = {
                let reference = &self.file.references[index];
                (
                    reference.kind,
                    reference.scope,
                    reference.range,
                    reference.name.clone(),
                )
            };

            let target = match kind {
                ReferenceKind::Name => self.resolve_name_at(scope, &name, range.start()),
                ReferenceKind::This | ReferenceKind::PathSegment | ReferenceKind::Field => None,
            };
            self.file.references[index].target = target;
            if let Some(target) = target {
                self.file.symbols[target.0 as usize]
                    .references
                    .push(crate::ReferenceId(index as u32));
            }
        }
    }

    pub(crate) fn resolve_value_flows(&mut self) {
        let pending_flows = std::mem::take(&mut self.pending_value_flows);
        for pending in pending_flows {
            let Some(symbol) = self.file.references[pending.reference.0 as usize].target else {
                continue;
            };
            self.push_value_flow(symbol, pending.expr, pending.kind, pending.range);
        }
    }

    pub(crate) fn resolve_mutations(&mut self) {
        let pending_mutations = std::mem::take(&mut self.pending_mutations);
        for pending in pending_mutations {
            let Some(symbol) = self.file.references[pending.receiver_reference.0 as usize].target
            else {
                continue;
            };

            let kind = match pending.kind {
                PendingMutationKind::Path { segments } => SymbolMutationKind::Path { segments },
            };

            self.file.symbol_mutations.push(SymbolMutation {
                symbol,
                value: pending.value,
                kind,
                range: pending.range,
            });
        }
    }

    pub(crate) fn first_reference_in_range(
        &self,
        range: rhai_syntax::TextRange,
    ) -> Option<crate::ReferenceId> {
        self.file
            .references
            .iter()
            .enumerate()
            .find_map(|(index, reference)| {
                (reference.range.start() >= range.start() && reference.range.end() <= range.end())
                    .then_some(crate::ReferenceId(index as u32))
            })
    }

    pub(crate) fn file_scope_id(&self) -> Option<ScopeId> {
        self.file
            .scopes
            .iter()
            .enumerate()
            .find_map(|(index, scope)| {
                (scope.kind == ScopeKind::File).then_some(ScopeId(index as u32))
            })
    }

    pub(crate) fn resolve_name_at(
        &self,
        mut scope: ScopeId,
        name: &str,
        reference_start: TextSize,
    ) -> Option<SymbolId> {
        let mut crossed_function_boundary = false;
        loop {
            if let Some(symbol) =
                self.resolve_name_in_scope(scope, name, reference_start, crossed_function_boundary)
            {
                return Some(symbol);
            }

            crossed_function_boundary |=
                self.file.scopes[scope.0 as usize].kind == ScopeKind::Function;
            scope = self.file.scopes[scope.0 as usize].parent?;
        }
    }

    pub(crate) fn resolve_name_in_scope(
        &self,
        scope: ScopeId,
        name: &str,
        reference_start: TextSize,
        crossed_function_boundary: bool,
    ) -> Option<SymbolId> {
        self.file.scopes[scope.0 as usize]
            .symbols
            .iter()
            .rev()
            .copied()
            .find(|symbol_id| {
                let symbol = &self.file.symbols[symbol_id.0 as usize];
                symbol.name == name
                    && self.symbol_is_visible_at(symbol, reference_start, crossed_function_boundary)
            })
    }

    pub(crate) fn symbol_is_visible_at(
        &self,
        symbol: &Symbol,
        reference_start: TextSize,
        crossed_function_boundary: bool,
    ) -> bool {
        if crossed_function_boundary {
            let symbol_scope = &self.file.scopes[symbol.scope.0 as usize];
            return symbol.kind == SymbolKind::Function
                || (symbol.kind == SymbolKind::ImportAlias
                    && symbol_scope.kind == ScopeKind::File
                    && symbol.range.start() <= reference_start);
        }

        matches!(symbol.kind, SymbolKind::Function) || symbol.range.start() <= reference_start
    }

    pub(crate) fn annotate_symbol_relationships(&mut self) {
        for symbol in &mut self.file.symbols {
            symbol.shadowed = None;
            symbol.duplicate_of = None;
        }

        for scope_index in 0..self.file.scopes.len() {
            let symbols = self.file.scopes[scope_index].symbols.clone();
            let mut seen = std::collections::HashMap::<String, SymbolId>::new();

            for symbol_id in symbols {
                let (name, start) = {
                    let symbol = &self.file.symbols[symbol_id.0 as usize];
                    (
                        self.symbol_relationship_key(symbol_id),
                        symbol.range.start(),
                    )
                };

                if let Some(previous) = seen.get(&name).copied() {
                    self.file.symbols[symbol_id.0 as usize].duplicate_of = Some(previous);
                    continue;
                }

                seen.insert(name.clone(), symbol_id);

                let shadowed = self.file.scopes[scope_index]
                    .parent
                    .and_then(|parent| self.resolve_name_at(parent, &name, start));
                self.file.symbols[symbol_id.0 as usize].shadowed = shadowed;
            }
        }
    }

    pub(crate) fn symbol_relationship_key(&self, symbol: SymbolId) -> String {
        let symbol_data = &self.file.symbols[symbol.0 as usize];
        if symbol_data.kind != SymbolKind::Function {
            return symbol_data.name.clone();
        }

        match self
            .file
            .function_infos
            .iter()
            .find(|info| info.symbol == symbol)
            .and_then(|info| info.this_type.as_ref())
        {
            Some(this_type) => {
                format!("{}#{}", symbol_data.name, self.function_type_key(this_type))
            }
            None => symbol_data.name.clone(),
        }
    }

    pub(crate) fn function_type_key(&self, this_type: &TypeRef) -> String {
        match this_type {
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
            TypeRef::Applied { name, .. } => name.clone(),
            TypeRef::Object(_) => "object".to_owned(),
            TypeRef::Array(_) => "array".to_owned(),
            TypeRef::Map(_, _) => "map".to_owned(),
            TypeRef::Nullable(inner) => format!("{}?", self.function_type_key(inner)),
            TypeRef::Union(items) => items
                .iter()
                .map(|item| self.function_type_key(item))
                .collect::<Vec<_>>()
                .join("|"),
            TypeRef::Function(_) => "fun".to_owned(),
        }
    }
}
