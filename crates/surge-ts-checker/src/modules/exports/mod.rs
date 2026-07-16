//! Building and resolving per-module export tables (named, default, star, namespace).

mod namespace;
mod promise;
mod statements;
mod table;
mod type_exports;
mod values;

pub(crate) use namespace::*;
pub(crate) use promise::*;
pub(crate) use statements::*;
pub(crate) use table::*;
pub(crate) use type_exports::*;
pub(crate) use values::*;

use super::*;

use std::sync::Arc;
use std::time::Instant;

use surge_ts_syntax::{
    ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedImportKind,
    ParsedNamespaceDeclaration, ParsedStatement, ParsedType, TextSpan,
};
use surge_ts_types::{FunctionType, ObjectProperty, PropertyMap, Type, TypeCopyReason};

use crate::checks::function as check_function;
use crate::checks::var::{VariableCheckOptions, check_variable_declaration_with_symbols};
use crate::context::{CheckerContext, FileKind};
use crate::program::{ParsedProgramFile, record_program_timing};
use crate::symbols::{
    SymbolInfo, SymbolKind, SymbolTable, TypeAliasInfo, TypeDeclarationInfo, TypeDeclarationScope,
    TypeDeclarationTable,
};

pub(crate) fn attach_type_resolution_scope(
    declaration: TypeDeclarationInfo,
    resolution_scope: Arc<TypeDeclarationScope>,
) -> TypeDeclarationInfo {
    match declaration {
        TypeDeclarationInfo::Alias(mut alias) => {
            if alias.resolution_scope.is_none() {
                alias.resolution_scope = Some(resolution_scope);
            }
            TypeDeclarationInfo::Alias(alias)
        }
        TypeDeclarationInfo::Interface(mut interface) => {
            if interface.resolution_scope.is_none() {
                interface.resolution_scope = Some(resolution_scope);
            }
            TypeDeclarationInfo::Interface(interface)
        }
    }
}

pub(crate) fn attach_type_resolution_scope_if_missing(
    declaration: TypeDeclarationInfo,
    resolution_scope: Option<&Arc<TypeDeclarationScope>>,
) -> TypeDeclarationInfo {
    match resolution_scope {
        Some(scope) => attach_type_resolution_scope(declaration, scope.clone()),
        None => declaration,
    }
}
