mod globals;
mod helpers;
mod methods;
mod types;

pub(crate) use crate::builtin::signatures::globals::register_builtin_global_functions;
pub(crate) use crate::builtin::signatures::methods::builtin_universal_method_names;
pub use crate::builtin::signatures::methods::builtin_universal_method_signature;
pub(crate) use crate::builtin::signatures::types::{builtin_host_types, host_type_name_for_type};
