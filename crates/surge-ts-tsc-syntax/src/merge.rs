//! The checker's merge of every file's global declarations into one symbol
//! table (`initializeChecker`, `mergeGlobalSymbol`, `mergeSymbol`,
//! `reportMergeSymbolError`), kept to the errors it reports: declarations in
//! different files that cannot share a name.
//!
//! Merges that need the checker's module resolution are left out: a module
//! augmentation of another module, and an alias whose target decides the
//! merge.

use std::collections::{HashMap, HashSet};

use crate::messages as diagnostics;
use crate::{Diagnostic, Message, SyntaxDiagnostic};

#[allow(non_upper_case_globals, dead_code)]
mod symbol_flags {
    pub const FunctionScopedVariable: u32 = 1 << 0;
    pub const BlockScopedVariable: u32 = 1 << 1;
    pub const Property: u32 = 1 << 2;
    pub const EnumMember: u32 = 1 << 3;
    pub const Function: u32 = 1 << 4;
    pub const Class: u32 = 1 << 5;
    pub const Interface: u32 = 1 << 6;
    pub const ConstEnum: u32 = 1 << 7;
    pub const RegularEnum: u32 = 1 << 8;
    pub const ValueModule: u32 = 1 << 9;
    pub const NamespaceModule: u32 = 1 << 10;
    pub const TypeLiteral: u32 = 1 << 11;
    pub const ObjectLiteral: u32 = 1 << 12;
    pub const Method: u32 = 1 << 13;
    pub const GetAccessor: u32 = 1 << 15;
    pub const SetAccessor: u32 = 1 << 16;
    pub const TypeParameter: u32 = 1 << 18;
    pub const TypeAlias: u32 = 1 << 19;
    pub const Alias: u32 = 1 << 21;
    pub const Assignment: u32 = 1 << 26;
    pub const ReplaceableByMethod: u32 = 1 << 29;

    pub const Enum: u32 = RegularEnum | ConstEnum;
    pub const Variable: u32 = FunctionScopedVariable | BlockScopedVariable;
    pub const Value: u32 = Variable
        | Property
        | EnumMember
        | ObjectLiteral
        | Function
        | Class
        | Enum
        | ValueModule
        | Method
        | GetAccessor
        | SetAccessor;
    pub const Type: u32 = Class | Interface | Enum | EnumMember | TypeLiteral | TypeParameter | TypeAlias;
    pub const Accessor: u32 = GetAccessor | SetAccessor;
    pub const Module: u32 = ValueModule | NamespaceModule;
    pub const Namespace: u32 = ValueModule | NamespaceModule | Enum;
}

use symbol_flags as sf;

/// The declarations one file contributes to the global scope, as its binder
/// declared them.
#[derive(Default, Debug)]
pub struct FileGlobals {
    pub(crate) symbols: Vec<GlobalSymbol>,
    /// A script's locals, which all join the global scope.
    pub(crate) is_script: bool,
    pub(crate) locals: Vec<u32>,
    /// A module's `declare global` blocks: the exports of each.
    pub(crate) augmentations: Vec<Vec<u32>>,
    /// A module's `declare module "…"` blocks: the name each augments, and
    /// the symbol the file's blocks of that name declare.
    pub(crate) module_augmentations: Vec<(String, u32)>,
}

#[derive(Default, Debug)]
pub(crate) struct GlobalSymbol {
    pub name: String,
    pub flags: u32,
    pub declarations: Vec<GlobalDeclaration>,
    pub members: Option<Vec<u32>>,
    pub exports: Option<Vec<u32>>,
}

#[derive(Debug)]
pub(crate) struct GlobalDeclaration {
    /// Where an error about the declaration goes: its name, or the node.
    pub name_range: (usize, usize),
    /// `createDiagnosticForNode(declaration)`.
    pub node_range: (usize, usize),
    pub is_type_declaration: bool,
    /// `rangeOfTypeParameters` of the declaration's type parameter list.
    pub type_parameters_range: Option<(usize, usize)>,
}

/// A file taking part in the merge, in program order.
pub struct GlobalsInput<'a> {
    pub globals: &'a FileGlobals,
    /// tsc's `IsPlainJSFile`: a merge conflict reports nothing here.
    pub plain_js: bool,
}

/// The errors the global merge reports, per file of `files`.
pub fn merge_globals(files: &[GlobalsInput<'_>]) -> Vec<Vec<SyntaxDiagnostic>> {
    merge_globals_report(files).diagnostics
}

/// What the global merge decides for each file of `files`.
pub struct MergeReport {
    /// The errors it reports.
    pub diagnostics: Vec<Vec<SyntaxDiagnostic>>,
    /// The type parameter lists of declarations whose symbol another file
    /// declares too: tsc checks a list for unused type parameters only when
    /// every declaration of its symbol is in one file
    /// (`allDeclarationsInSameSourceFile`).
    pub shared_type_parameters: Vec<Vec<(usize, usize)>>,
}

pub fn merge_globals_report(files: &[GlobalsInput<'_>]) -> MergeReport {
    let mut merger = Merger {
        files,
        merged: Vec::new(),
        merged_symbols: HashMap::new(),
        reported: Vec::new(),
        seen: HashSet::new(),
    };
    let mut globals = Table::default();
    let mut ambient_modules = Vec::new();
    for (file, input) in files.iter().enumerate() {
        if !input.globals.is_script {
            continue;
        }
        for &local in &input.globals.locals {
            let global = &input.globals.symbols[local as usize];
            if global.name == "globalThis" {
                for declaration in &global.declarations {
                    merger.report(
                        file as u32,
                        declaration.node_range,
                        diagnostics::Declaration_name_conflicts_with_built_in_global_identifier_0,
                        vec!["globalThis".into()],
                    );
                }
            }
            let symbol = Sym::File(file as u32, local);
            // Global ambient module declarations merge after the rest.
            if global.flags & sf::Module != 0 && global.name.starts_with('"') {
                ambient_modules.push(symbol);
                continue;
            }
            merger.merge_global_symbol(&mut globals, symbol);
        }
    }
    for (file, input) in files.iter().enumerate() {
        for exports in &input.globals.augmentations {
            let source: Vec<(String, Sym)> = exports
                .iter()
                .map(|&id| (input.globals.symbols[id as usize].name.clone(), Sym::File(file as u32, id)))
                .collect();
            merger.merge_into_table(TableRef::Globals, &mut globals, source, false);
        }
    }
    if let Some(&undefined) = globals.index.get("undefined") {
        let declarations = merger.declarations(globals.entries[undefined].1);
        for (file, declaration) in declarations {
            if !declaration.is_type_declaration {
                merger.report(
                    file,
                    declaration.node_range,
                    diagnostics::Declaration_name_conflicts_with_built_in_global_identifier_0,
                    vec!["undefined".into()],
                );
            }
        }
    }
    for symbol in ambient_modules {
        merger.merge_global_symbol(&mut globals, symbol);
    }
    for (file, input) in files.iter().enumerate() {
        for (module_name, id) in &input.globals.module_augmentations {
            // port: only an augmentation of an ambient module is merged; one
            // of a module file needs the checker's module resolution.
            let Some(main_module) = merger.try_find_ambient_module(&globals, module_name) else { continue };
            if merger.flags(main_module) & sf::Namespace != 0 {
                merger.merge_symbol(main_module, Sym::File(file as u32, *id), false);
            }
        }
    }
    let mut shared_type_parameters = vec![Vec::new(); files.len()];
    for symbol in &merger.merged {
        let first_file = symbol.declarations.first().map(|&(file, _, _)| file);
        if symbol.declarations.iter().all(|&(file, _, _)| Some(file) == first_file) {
            continue;
        }
        for &(file, id, index) in &symbol.declarations {
            if let Some(range) = merger.file_symbol(file, id).declarations[index].type_parameters_range {
                shared_type_parameters[file as usize].push(range);
            }
        }
    }
    let mut diagnostics = vec![Vec::new(); files.len()];
    for (file, diagnostic) in merger.reported {
        diagnostics[file as usize].push(diagnostic.render());
    }
    MergeReport { diagnostics, shared_type_parameters }
}

/// A symbol of one file, or one the merge made (tsc's transient clone).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Sym {
    File(u32, u32),
    Merged(u32),
}

/// A symbol table that keeps its insertion order.
#[derive(Default, Clone)]
struct Table {
    entries: Vec<(String, Sym)>,
    index: HashMap<String, usize>,
}

impl Table {
    fn get(&self, name: &str) -> Option<Sym> {
        self.index.get(name).map(|&i| self.entries[i].1)
    }

    fn set(&mut self, name: String, symbol: Sym) {
        match self.index.get(&name) {
            Some(&i) => self.entries[i].1 = symbol,
            None => {
                self.index.insert(name.clone(), self.entries.len());
                self.entries.push((name, symbol));
            }
        }
    }
}

struct MergedSymbol {
    name: String,
    flags: u32,
    declarations: Vec<(u32, u32, usize)>,
    members: Option<Table>,
    exports: Option<Table>,
}

#[derive(Clone, Copy)]
enum TableRef {
    Globals,
    Members(u32),
    Exports(u32),
}

struct Merger<'a> {
    files: &'a [GlobalsInput<'a>],
    merged: Vec<MergedSymbol>,
    merged_symbols: HashMap<Sym, Sym>,
    reported: Vec<(u32, Diagnostic)>,
    seen: HashSet<(u32, usize, usize, u32, Vec<String>)>,
}

impl<'a> Merger<'a> {
    fn file_symbol(&self, file: u32, id: u32) -> &'a GlobalSymbol {
        &self.files[file as usize].globals.symbols[id as usize]
    }

    fn flags(&self, symbol: Sym) -> u32 {
        match symbol {
            Sym::File(file, id) => self.file_symbol(file, id).flags,
            Sym::Merged(id) => self.merged[id as usize].flags,
        }
    }

    fn name(&self, symbol: Sym) -> String {
        match symbol {
            Sym::File(file, id) => self.file_symbol(file, id).name.clone(),
            Sym::Merged(id) => self.merged[id as usize].name.clone(),
        }
    }

    fn declaration_refs(&self, symbol: Sym) -> Vec<(u32, u32, usize)> {
        match symbol {
            Sym::File(file, id) => (0..self.file_symbol(file, id).declarations.len()).map(|i| (file, id, i)).collect(),
            Sym::Merged(id) => self.merged[id as usize].declarations.clone(),
        }
    }

    fn declarations(&self, symbol: Sym) -> Vec<(u32, &'a GlobalDeclaration)> {
        self.declaration_refs(symbol)
            .into_iter()
            .map(|(file, id, i)| (file, &self.file_symbol(file, id).declarations[i]))
            .collect()
    }

    fn file_table(&self, file: u32, ids: &Option<Vec<u32>>) -> Option<Table> {
        let ids = ids.as_ref()?;
        let mut table = Table::default();
        for &id in ids {
            table.set(self.file_symbol(file, id).name.clone(), Sym::File(file, id));
        }
        Some(table)
    }

    fn members(&self, symbol: Sym) -> Option<Table> {
        match symbol {
            Sym::File(file, id) => self.file_table(file, &self.file_symbol(file, id).members),
            Sym::Merged(id) => self.merged[id as usize].members.clone(),
        }
    }

    fn exports(&self, symbol: Sym) -> Option<Table> {
        match symbol {
            Sym::File(file, id) => self.file_table(file, &self.file_symbol(file, id).exports),
            Sym::Merged(id) => self.merged[id as usize].exports.clone(),
        }
    }

    fn merged_symbol(&self, symbol: Sym) -> Sym {
        self.merged_symbols.get(&symbol).copied().unwrap_or(symbol)
    }

    /// `tryFindAmbientModule(moduleName, withAugmentations: true)`.
    fn try_find_ambient_module(&self, globals: &Table, module_name: &str) -> Option<Sym> {
        if is_external_module_name_relative(module_name) {
            return None;
        }
        let symbol = globals.get(&format!("\"{module_name}\""))?;
        (self.flags(symbol) & sf::ValueModule != 0).then(|| self.merged_symbol(symbol))
    }

    fn merge_global_symbol(&mut self, globals: &mut Table, symbol: Sym) {
        let name = self.name(symbol);
        let merged = match globals.get(&name) {
            Some(global) => self.merge_symbol(global, symbol, false),
            None => self.merged_symbol(symbol),
        };
        globals.set(name, merged);
    }

    /// `mergeSymbolTable` into a table this merge owns (the globals, or a
    /// merged symbol's members or exports).
    fn merge_into_table(&mut self, target: TableRef, globals: &mut Table, source: Vec<(String, Sym)>, unidirectional: bool) {
        for (name, source_symbol) in source {
            let existing = match target {
                TableRef::Globals => globals.get(&name),
                TableRef::Members(id) => self.merged[id as usize].members.as_ref().and_then(|t| t.get(&name)),
                TableRef::Exports(id) => self.merged[id as usize].exports.as_ref().and_then(|t| t.get(&name)),
            };
            let merged = match existing {
                Some(target_symbol) => self.merge_symbol(target_symbol, source_symbol, unidirectional),
                None => self.merged_symbol(source_symbol),
            };
            match target {
                TableRef::Globals => globals.set(name, merged),
                TableRef::Members(id) => self.merged[id as usize].members.get_or_insert_with(Table::default).set(name, merged),
                TableRef::Exports(id) => self.merged[id as usize].exports.get_or_insert_with(Table::default).set(name, merged),
            }
        }
    }

    fn clone_symbol(&mut self, symbol: Sym) -> u32 {
        let clone = MergedSymbol {
            name: self.name(symbol),
            flags: self.flags(symbol),
            declarations: self.declaration_refs(symbol),
            members: self.members(symbol),
            exports: self.exports(symbol),
        };
        let id = self.merged.len() as u32;
        self.merged.push(clone);
        self.merged_symbols.insert(symbol, Sym::Merged(id));
        id
    }

    fn merge_symbol(&mut self, target: Sym, source: Sym, unidirectional: bool) -> Sym {
        let target_flags = self.flags(target);
        let source_flags = self.flags(source);
        if target_flags & excluded_symbol_flags(source_flags) == 0 || (source_flags | target_flags) & sf::Assignment != 0 {
            if source == target {
                return target;
            }
            let target_id = match target {
                Sym::Merged(id) => id,
                Sym::File(..) => {
                    // port: `resolveSymbol` needs the checker's alias
                    // resolution; an alias merges as tsc's does with an
                    // unresolved one.
                    if target_flags & sf::Alias != 0 {
                        return source;
                    }
                    self.clone_symbol(target)
                }
            };
            self.merged[target_id as usize].flags |= source_flags;
            let declarations = self.declaration_refs(source);
            self.merged[target_id as usize].declarations.extend(declarations);
            if let Some(members) = self.members(source) {
                let mut globals = Table::default();
                self.merged[target_id as usize].members.get_or_insert_with(Table::default);
                self.merge_into_table(TableRef::Members(target_id), &mut globals, members.entries, unidirectional);
            }
            if let Some(exports) = self.exports(source) {
                let mut globals = Table::default();
                self.merged[target_id as usize].exports.get_or_insert_with(Table::default);
                self.merge_into_table(TableRef::Exports(target_id), &mut globals, exports.entries, unidirectional);
            }
            if !unidirectional {
                self.merged_symbols.insert(source, Sym::Merged(target_id));
            }
            return Sym::Merged(target_id);
        }
        if target_flags & sf::NamespaceModule != 0 {
            // port: TS2649 (augmenting a non-module entity) is not reported.
            return target;
        }
        self.report_merge_symbol_error(target, source);
        target
    }

    fn report_merge_symbol_error(&mut self, target: Sym, source: Sym) {
        let target_flags = self.flags(target);
        let source_flags = self.flags(source);
        let message = if target_flags & sf::Enum != 0 || source_flags & sf::Enum != 0 {
            diagnostics::Enum_declarations_can_only_merge_with_namespace_or_other_enum_declarations
        } else if target_flags & sf::BlockScopedVariable != 0 || source_flags & sf::BlockScopedVariable != 0 {
            diagnostics::Cannot_redeclare_block_scoped_variable_0
        } else {
            diagnostics::Duplicate_identifier_0
        };
        let symbol_name = symbol_to_string(&self.name(source));
        let first_file = |merger: &Self, symbol: Sym| merger.declaration_refs(symbol).first().map(|&(file, _, _)| file);
        let source_plain_js = first_file(self, source).is_some_and(|file| self.files[file as usize].plain_js);
        let target_plain_js = first_file(self, target).is_some_and(|file| self.files[file as usize].plain_js);
        let args = if message.text.contains("{0}") { vec![symbol_name] } else { Vec::new() };
        if !source_plain_js {
            for (file, declaration) in self.declarations(source) {
                self.report(file, declaration.name_range, message, args.clone());
            }
        }
        if !target_plain_js {
            for (file, declaration) in self.declarations(target) {
                self.report(file, declaration.name_range, message, args.clone());
            }
        }
    }

    /// `lookupOrIssueError`: one report per place and message.
    fn report(&mut self, file: u32, (start, end): (usize, usize), message: &'static Message, args: Vec<String>) {
        if self.seen.insert((file, start, end, message.code, args.clone())) {
            self.reported.push((file, Diagnostic { start, end, message, args }));
        }
    }
}

/// `tspath.IsExternalModuleNameRelative`: a relative path, or a rooted one.
fn is_external_module_name_relative(name: &str) -> bool {
    let bytes = name.as_bytes();
    name == "."
        || name == ".."
        || name.starts_with("./")
        || name.starts_with(".\\")
        || name.starts_with("../")
        || name.starts_with("..\\")
        || matches!(bytes.first(), Some(b'/' | b'\\'))
        || bytes.len() >= 2
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && (bytes.len() == 2 || matches!(bytes[2], b'/' | b'\\'))
        || name.starts_with("^/")
        || name.contains("://")
}

/// `symbolToString` for a global's name.
fn symbol_to_string(name: &str) -> String {
    match name.strip_prefix('\u{FE}') {
        Some(rest) => format!("__{rest}"),
        None => name.to_string(),
    }
}

/// `getExcludedSymbolFlags`.
fn excluded_symbol_flags(flags: u32) -> u32 {
    let mut result = 0;
    let pairs: [(u32, u32); 16] = [
        (sf::BlockScopedVariable, sf::Value),
        (sf::FunctionScopedVariable, sf::Value & !sf::FunctionScopedVariable),
        (sf::Property, sf::Value & !(sf::Property | sf::Accessor)),
        (sf::EnumMember, sf::Value | sf::Type),
        (sf::Function, sf::Value & !(sf::Function | sf::ValueModule | sf::Class)),
        (sf::Class, (sf::Value | sf::Type) & !(sf::ValueModule | sf::Interface | sf::Function)),
        (sf::Interface, sf::Type & !(sf::Interface | sf::Class)),
        (sf::RegularEnum, (sf::Value | sf::Type) & !(sf::RegularEnum | sf::ValueModule)),
        (sf::ConstEnum, (sf::Value | sf::Type) & !sf::ConstEnum),
        (sf::ValueModule, sf::Value & !(sf::Function | sf::Class | sf::RegularEnum | sf::ValueModule)),
        (sf::Method, sf::Value & !sf::Method),
        (sf::GetAccessor, sf::Value & !(sf::SetAccessor | sf::Property)),
        (sf::SetAccessor, sf::Value & !(sf::GetAccessor | sf::Property)),
        (sf::TypeParameter, sf::Type & !sf::TypeParameter),
        (sf::TypeAlias, sf::Type),
        (sf::Alias, sf::Alias),
    ];
    for (flag, excludes) in pairs {
        if flags & flag != 0 {
            result |= excludes;
        }
    }
    if flags & sf::ReplaceableByMethod != 0 {
        result &= !sf::Method;
    }
    result
}
