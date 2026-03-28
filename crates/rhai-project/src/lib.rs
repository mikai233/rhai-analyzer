use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub root: PathBuf,
    pub source_roots: Vec<PathBuf>,
    pub engine: EngineOptions,
    pub modules: BTreeMap<String, ModuleSpec>,
    pub types: BTreeMap<String, TypeSpec>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngineOptions {
    pub disabled_symbols: Vec<String>,
    pub custom_syntaxes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModuleSpec {
    pub docs: Option<String>,
    pub functions: BTreeMap<String, Vec<FunctionSpec>>,
    pub constants: BTreeMap<String, ConstantSpec>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FunctionSpec {
    pub signature: String,
    pub return_type: Option<String>,
    pub docs: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConstantSpec {
    pub type_name: String,
    pub docs: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypeSpec {
    pub docs: Option<String>,
    pub methods: BTreeMap<String, Vec<FunctionSpec>>,
}
