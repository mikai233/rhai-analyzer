use std::cmp::Ordering;

use rhai_hir::{FunctionTypeRef, TypeRef};

pub fn best_matching_signature_indexes<'a, I>(
    signatures: I,
    arg_types: &[Option<TypeRef>],
) -> Vec<usize>
where
    I: IntoIterator<Item = &'a FunctionTypeRef>,
{
    let signatures = signatures.into_iter().collect::<Vec<_>>();
    let mut best_score = None::<SignatureScore>;
    let mut best_indexes = Vec::new();

    for (index, signature) in signatures.iter().enumerate() {
        if signature.params.len() != arg_types.len() {
            continue;
        }

        let score = signature_match_score(signature, arg_types);
        match best_score {
            None => {
                best_score = Some(score);
                best_indexes.push(index);
            }
            Some(current) if score > current => {
                best_score = Some(score);
                best_indexes.clear();
                best_indexes.push(index);
            }
            Some(current) if score == current => {
                best_indexes.push(index);
            }
            Some(_) => {}
        }
    }

    best_indexes
}

pub fn best_matching_signature_index<'a, I>(
    signatures: I,
    arg_types: &[Option<TypeRef>],
) -> Option<usize>
where
    I: IntoIterator<Item = &'a FunctionTypeRef>,
{
    best_matching_signature_indexes(signatures, arg_types)
        .into_iter()
        .next()
}

pub fn signature_match_quality(
    signature: &FunctionTypeRef,
    arg_types: &[Option<TypeRef>],
) -> Option<SignatureMatchQuality> {
    (signature.params.len() == arg_types.len())
        .then(|| signature_match_score(signature, arg_types).quality())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SignatureScore {
    mismatch_free: bool,
    exact: usize,
    partial: usize,
    unknown: usize,
    mismatched: usize,
}

impl SignatureScore {
    fn quality(self) -> SignatureMatchQuality {
        if !self.mismatch_free {
            return SignatureMatchQuality::Mismatch;
        }

        let compared = self.exact + self.partial + self.unknown;
        if compared > 0 && self.exact == compared {
            return SignatureMatchQuality::Exact;
        }

        if self.exact > 0 || self.partial > 0 {
            return SignatureMatchQuality::Partial;
        }

        SignatureMatchQuality::Unknown
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignatureMatchQuality {
    Mismatch,
    Unknown,
    Partial,
    Exact,
}

impl Ord for SignatureScore {
    fn cmp(&self, other: &Self) -> Ordering {
        self.mismatch_free
            .cmp(&other.mismatch_free)
            .then_with(|| self.exact.cmp(&other.exact))
            .then_with(|| self.partial.cmp(&other.partial))
            .then_with(|| other.mismatched.cmp(&self.mismatched))
            .then_with(|| other.unknown.cmp(&self.unknown))
    }
}

impl PartialOrd for SignatureScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn signature_match_score(
    signature: &FunctionTypeRef,
    arg_types: &[Option<TypeRef>],
) -> SignatureScore {
    let mut score = SignatureScore {
        mismatch_free: true,
        exact: 0,
        partial: 0,
        unknown: 0,
        mismatched: 0,
    };

    for (param, arg_ty) in signature.params.iter().zip(arg_types.iter()) {
        match arg_ty {
            None => score.unknown += 1,
            Some(arg_ty) => match param_match_score(param, arg_ty) {
                ParamMatchScore::Exact => score.exact += 1,
                ParamMatchScore::Partial => score.partial += 1,
                ParamMatchScore::Unknown => score.unknown += 1,
                ParamMatchScore::Mismatch => {
                    score.mismatch_free = false;
                    score.mismatched += 1;
                }
            },
        }
    }

    score
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ParamMatchScore {
    Mismatch,
    Unknown,
    Partial,
    Exact,
}

fn param_match_score(expected: &TypeRef, actual: &TypeRef) -> ParamMatchScore {
    if expected == actual {
        return ParamMatchScore::Exact;
    }

    match (expected, actual) {
        (TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic | TypeRef::Never, _)
        | (_, TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic | TypeRef::Never) => {
            ParamMatchScore::Unknown
        }
        (TypeRef::Ambiguous(items), other) | (other, TypeRef::Ambiguous(items)) => items
            .iter()
            .map(|item| param_match_score(item, other))
            .max()
            .unwrap_or(ParamMatchScore::Mismatch),
        (TypeRef::Union(items), other) | (other, TypeRef::Union(items)) => items
            .iter()
            .map(|item| param_match_score(item, other))
            .max()
            .unwrap_or(ParamMatchScore::Mismatch),
        (TypeRef::Nullable(left), TypeRef::Nullable(right)) => {
            downgrade_nested_match(param_match_score(left, right))
        }
        (TypeRef::Nullable(inner), other) | (other, TypeRef::Nullable(inner)) => {
            let nested = param_match_score(inner, other);
            if nested == ParamMatchScore::Exact {
                ParamMatchScore::Partial
            } else {
                nested
            }
        }
        (TypeRef::Named(left), TypeRef::Named(right)) if left == right => ParamMatchScore::Exact,
        (TypeRef::Named(left), TypeRef::Applied { name, .. })
        | (TypeRef::Applied { name, .. }, TypeRef::Named(left))
            if left == name =>
        {
            ParamMatchScore::Partial
        }
        (
            TypeRef::Applied {
                name: left_name,
                args: left_args,
            },
            TypeRef::Applied {
                name: right_name,
                args: right_args,
            },
        ) if left_name == right_name && left_args.len() == right_args.len() => {
            aggregate_nested_match_scores(
                left_args
                    .iter()
                    .zip(right_args.iter())
                    .map(|(left, right)| param_match_score(left, right)),
            )
        }
        (TypeRef::Array(left), TypeRef::Array(right)) => {
            downgrade_nested_match(param_match_score(left, right))
        }
        (TypeRef::Object(left), TypeRef::Object(right)) => aggregate_nested_match_scores(
            left.iter()
                .filter_map(|(name, left_ty)| right.get(name).map(|right_ty| (left_ty, right_ty)))
                .map(|(left_ty, right_ty)| param_match_score(left_ty, right_ty)),
        ),
        (TypeRef::Map(left_key, left_value), TypeRef::Map(right_key, right_value)) => {
            aggregate_nested_match_scores([
                param_match_score(left_key, right_key),
                param_match_score(left_value, right_value),
            ])
        }
        (
            TypeRef::Function(FunctionTypeRef {
                params: left_params,
                ret: left_ret,
            }),
            TypeRef::Function(FunctionTypeRef {
                params: right_params,
                ret: right_ret,
            }),
        ) if left_params.len() == right_params.len() => aggregate_nested_match_scores(
            left_params
                .iter()
                .zip(right_params.iter())
                .map(|(left, right)| param_match_score(left, right))
                .chain(std::iter::once(param_match_score(left_ret, right_ret))),
        ),
        _ => ParamMatchScore::Mismatch,
    }
}

fn aggregate_nested_match_scores<I>(scores: I) -> ParamMatchScore
where
    I: IntoIterator<Item = ParamMatchScore>,
{
    let mut saw_partial = false;
    let mut saw_unknown = false;

    for score in scores {
        match score {
            ParamMatchScore::Mismatch => return ParamMatchScore::Mismatch,
            ParamMatchScore::Unknown => saw_unknown = true,
            ParamMatchScore::Partial => saw_partial = true,
            ParamMatchScore::Exact => {}
        }
    }

    if saw_partial {
        ParamMatchScore::Partial
    } else if saw_unknown {
        ParamMatchScore::Unknown
    } else {
        ParamMatchScore::Exact
    }
}

fn downgrade_nested_match(score: ParamMatchScore) -> ParamMatchScore {
    match score {
        ParamMatchScore::Exact => ParamMatchScore::Partial,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use rhai_hir::{FunctionTypeRef, TypeRef};

    use super::{best_matching_signature_index, best_matching_signature_indexes};

    #[test]
    fn prefers_exact_over_same_arity_mismatch() {
        let signatures = [
            FunctionTypeRef {
                params: vec![TypeRef::Int],
                ret: Box::new(TypeRef::Int),
            },
            FunctionTypeRef {
                params: vec![TypeRef::String],
                ret: Box::new(TypeRef::Bool),
            },
        ];

        assert_eq!(
            best_matching_signature_index(signatures.iter(), &[Some(TypeRef::String)]),
            Some(1)
        );
    }

    #[test]
    fn keeps_arity_selection_when_argument_types_are_unknown() {
        let signatures = [
            FunctionTypeRef {
                params: vec![TypeRef::Int],
                ret: Box::new(TypeRef::Int),
            },
            FunctionTypeRef {
                params: vec![TypeRef::String],
                ret: Box::new(TypeRef::Bool),
            },
        ];

        assert_eq!(
            best_matching_signature_index(signatures.iter(), &[None]),
            Some(0)
        );
    }

    #[test]
    fn keeps_tied_best_signature_indexes_for_ambiguous_overloads() {
        let signatures = [
            FunctionTypeRef {
                params: vec![TypeRef::Int],
                ret: Box::new(TypeRef::Int),
            },
            FunctionTypeRef {
                params: vec![TypeRef::String],
                ret: Box::new(TypeRef::String),
            },
        ];

        assert_eq!(
            best_matching_signature_indexes(
                signatures.iter(),
                &[Some(TypeRef::Union(vec![TypeRef::Int, TypeRef::String]))]
            ),
            vec![0, 1]
        );
    }
}
