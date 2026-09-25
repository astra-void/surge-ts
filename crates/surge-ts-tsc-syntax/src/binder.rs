//! The diagnostics of typescript-go's binder (`internal/binder/binder.go`):
//! conflicting declarations within one file, and the strict-mode restrictions
//! it checks while it walks the tree. Control flow is not built; nothing it
//! decides is reported by the binder.

use std::collections::HashMap;

use crate::ast::{self, NodeId};
use crate::flags::NodeFlags;
use crate::kind::Kind;
use crate::merge::{FileGlobals, GlobalDeclaration, GlobalSymbol};
use crate::messages as diagnostics;
use crate::parser::ParsedFile;
use crate::scanner::{self, Scanner};
use crate::{Diagnostic, Message};

#[allow(non_upper_case_globals, dead_code)]
mod symbol_flags {
    pub const None: u32 = 0;
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
    pub const Constructor: u32 = 1 << 14;
    pub const GetAccessor: u32 = 1 << 15;
    pub const SetAccessor: u32 = 1 << 16;
    pub const Signature: u32 = 1 << 17;
    pub const TypeParameter: u32 = 1 << 18;
    pub const TypeAlias: u32 = 1 << 19;
    pub const ExportValue: u32 = 1 << 20;
    pub const Alias: u32 = 1 << 21;
    pub const Prototype: u32 = 1 << 22;
    pub const ExportStar: u32 = 1 << 23;
    pub const Optional: u32 = 1 << 24;
    pub const Assignment: u32 = 1 << 26;
    pub const ReplaceableByMethod: u32 = 1 << 29;
    pub const All: u32 = (1 << 30) - 1;

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
    pub const Namespace: u32 = ValueModule | NamespaceModule | Enum;
    pub const Accessor: u32 = GetAccessor | SetAccessor;
    pub const ClassMember: u32 = Method | Accessor | Property;

    pub const FunctionScopedVariableExcludes: u32 = Value & !FunctionScopedVariable;
    pub const BlockScopedVariableExcludes: u32 = Value;
    pub const ParameterExcludes: u32 = Value;
    pub const PropertyExcludes: u32 = Value & !(Property | Accessor);
    pub const EnumMemberExcludes: u32 = Value | Type;
    pub const FunctionExcludes: u32 = Value & !(Function | ValueModule | Class);
    pub const ClassExcludes: u32 = (Value | Type) & !(ValueModule | Interface | Function);
    pub const InterfaceExcludes: u32 = Type & !(Interface | Class);
    pub const RegularEnumExcludes: u32 = (Value | Type) & !(RegularEnum | ValueModule);
    pub const ConstEnumExcludes: u32 = (Value | Type) & !ConstEnum;
    pub const ValueModuleExcludes: u32 = Value & !(Function | Class | RegularEnum | ValueModule);
    pub const NamespaceModuleExcludes: u32 = None;
    pub const MethodExcludes: u32 = Value & !Method;
    pub const GetAccessorExcludes: u32 = Value & !(SetAccessor | Property);
    pub const SetAccessorExcludes: u32 = Value & !(GetAccessor | Property);
    pub const AccessorExcludes: u32 = Value & !Property;
    pub const TypeParameterExcludes: u32 = Type & !TypeParameter;
    pub const TypeAliasExcludes: u32 = Type;
    pub const AliasExcludes: u32 = Alias;
}

use symbol_flags as sf;

const INTERNAL_PREFIX: &str = "\u{FE}";

fn internal(name: &str) -> String {
    format!("{INTERNAL_PREFIX}{name}")
}

const MISSING: &str = "\u{FE}missing";
const DEFAULT: &str = "default";
const EXPORT_EQUALS: &str = "export=";

#[allow(non_upper_case_globals, dead_code)]
mod container_flags {
    pub const None: u32 = 0;
    pub const IsContainer: u32 = 1 << 0;
    pub const IsBlockScopedContainer: u32 = 1 << 1;
    pub const IsControlFlowContainer: u32 = 1 << 2;
    pub const IsFunctionLike: u32 = 1 << 3;
    pub const IsFunctionExpression: u32 = 1 << 4;
    pub const HasLocals: u32 = 1 << 5;
    pub const IsInterface: u32 = 1 << 6;
    pub const IsObjectLiteralOrClassExpressionMethodOrAccessor: u32 = 1 << 7;
    pub const IsThisContainer: u32 = 1 << 8;
    pub const PropagatesThisKeyword: u32 = 1 << 9;
}

use container_flags as cf;

mod unused;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ModuleInstanceState {
    Unknown,
    NonInstantiated,
    Instantiated,
    ConstEnumOnly,
}

type SymbolId = usize;

struct Symbol {
    flags: u32,
    declarations: Vec<NodeId>,
    /// `Symbol.ExportSymbol`: an exported declaration's local symbol names
    /// the symbol its container exports.
    export_symbol: Option<SymbolId>,
    value_declaration: Option<NodeId>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Table {
    Locals(NodeId),
    Exports(SymbolId),
    Members(SymbolId),
}

/// Binds the file: what the binder reports and what [`Binder::finish`] and
/// the unused-identifier check read back.
pub(crate) fn bind<'a>(file: &'a ParsedFile, text: &'a str) -> Binder<'a> {
    let mut binder = Binder {
        file,
        text,
        jsx: file.jsx,
        symbols: Vec::new(),
        tables: HashMap::new(),
        node_symbol: HashMap::new(),
        local_symbol: HashMap::new(),
        export_context: Vec::new(),
        container: file.root,
        block_scope_container: file.root,
        diagnostics: Vec::new(),
    };
    binder.bind(Some(file.root));
    binder
}

pub(crate) struct Binder<'a> {
    file: &'a ParsedFile,
    text: &'a str,
    jsx: bool,
    symbols: Vec<Symbol>,
    tables: HashMap<Table, HashMap<String, SymbolId>>,
    /// `node.Symbol()`: the symbol a declaration was last added to.
    node_symbol: HashMap<NodeId, SymbolId>,
    /// `node.LocalSymbol()`: an exported declaration's symbol in its
    /// container's locals.
    local_symbol: HashMap<NodeId, SymbolId>,
    /// `NodeFlagsExportContext`, which the binder sets on a source file or
    /// module declaration.
    export_context: Vec<NodeId>,
    container: NodeId,
    block_scope_container: NodeId,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Binder<'a> {
    /// The file's binder diagnostics, and the symbols it contributes to the
    /// program's global scope.
    pub(crate) fn finish(self) -> (Vec<Diagnostic>, FileGlobals) {
        let globals = self.globals();
        (self.diagnostics, globals)
    }

    fn kind(&self, id: NodeId) -> Kind {
        self.file.node(id).kind
    }

    fn flags(&self, id: NodeId) -> NodeFlags {
        self.file.node(id).flags
    }

    fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.file.parent(id)
    }

    fn node_text(&self, id: NodeId) -> String {
        let node = self.file.node(id);
        match node.kind {
            Kind::JsxNamespacedName => {
                let namespace = node.expression.map(|n| self.node_text(n)).unwrap_or_default();
                let name = node.name.map(|n| self.node_text(n)).unwrap_or_default();
                format!("{namespace}:{name}")
            }
            _ => node.text.clone(),
        }
    }

    fn statements(&self, id: NodeId) -> Vec<NodeId> {
        self.file.node(id).lists.first().and_then(|list| list.as_ref()).map_or_else(Vec::new, |list| list.nodes.clone())
    }

    fn new_symbol(&mut self, flags: u32) -> SymbolId {
        self.symbols.push(Symbol { flags, declarations: Vec::new(), export_symbol: None, value_declaration: None });
        self.symbols.len() - 1
    }

    fn table(&mut self, table: Table) -> &mut HashMap<String, SymbolId> {
        self.tables.entry(table).or_default()
    }

    fn symbol_of(&self, node: NodeId) -> Option<SymbolId> {
        self.node_symbol.get(&node).copied()
    }

    fn exports_of(&self, node: NodeId) -> Option<Table> {
        self.symbol_of(node).map(Table::Exports)
    }

    fn members_of(&self, node: NodeId) -> Option<Table> {
        self.symbol_of(node).map(Table::Members)
    }

    // --- modifiers -----------------------------------------------------------

    fn has_modifier(&self, node: NodeId, kind: Kind) -> bool {
        self.file.node(node).modifier_nodes().iter().any(|&m| self.kind(m) == kind)
    }

    fn root_declaration(&self, mut node: NodeId) -> NodeId {
        while self.kind(node) == Kind::BindingElement {
            match self.parent(node).and_then(|p| self.parent(p)) {
                Some(declaration) => node = declaration,
                None => break,
            }
        }
        node
    }

    /// `getCombinedFlags` over modifiers: a variable declaration takes its
    /// statement's modifiers.
    fn combined_has_modifier(&self, node: NodeId, kind: Kind) -> bool {
        let mut node = self.root_declaration(node);
        if self.has_modifier(node, kind) {
            return true;
        }
        if self.kind(node) == Kind::VariableDeclaration {
            match self.parent(node) {
                Some(parent) => node = parent,
                None => return false,
            }
        }
        if self.kind(node) == Kind::VariableDeclarationList {
            if self.has_modifier(node, kind) {
                return true;
            }
            match self.parent(node) {
                Some(parent) => node = parent,
                None => return false,
            }
        }
        self.kind(node) == Kind::VariableStatement && self.has_modifier(node, kind)
    }

    fn combined_node_flags(&self, node: NodeId) -> NodeFlags {
        let mut node = self.root_declaration(node);
        let mut flags = self.flags(node);
        if self.kind(node) == Kind::VariableDeclaration {
            match self.parent(node) {
                Some(parent) => node = parent,
                None => return flags,
            }
        }
        if self.kind(node) == Kind::VariableDeclarationList {
            flags |= self.flags(node);
            match self.parent(node) {
                Some(parent) => node = parent,
                None => return flags,
            }
        }
        if self.kind(node) == Kind::VariableStatement {
            flags |= self.flags(node);
        }
        flags
    }

    // --- ast utilities -------------------------------------------------------

    fn name_of(&self, node: NodeId) -> Option<NodeId> {
        self.file.node(node).name
    }

    /// `ast.GetNameOfDeclaration` for the declarations the binder declares.
    fn name_of_declaration(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            Kind::BinaryExpression | Kind::CallExpression => None,
            Kind::ExportAssignment => {
                let expression = self.file.node(node).expression?;
                (self.kind(expression) == Kind::Identifier).then_some(expression)
            }
            _ => self.name_of(node),
        }
    }

    fn is_global_scope_augmentation(&self, node: NodeId) -> bool {
        self.kind(node) == Kind::ModuleDeclaration && self.file.node(node).op == Kind::GlobalKeyword
    }

    fn is_ambient_module(&self, node: NodeId) -> bool {
        self.kind(node) == Kind::ModuleDeclaration
            && (self.name_of(node).is_some_and(|name| self.kind(name) == Kind::StringLiteral)
                || self.is_global_scope_augmentation(node))
    }

    fn is_module_augmentation_external(&self, node: NodeId) -> bool {
        let Some(parent) = self.parent(node) else { return false };
        match self.kind(parent) {
            Kind::SourceFile => self.file.external_module,
            Kind::ModuleBlock => self.parent(parent).is_some_and(|grand_parent| {
                self.is_ambient_module(grand_parent)
                    && self.parent(grand_parent).is_some_and(|p| self.kind(p) == Kind::SourceFile)
                    && !self.file.external_module
            }),
            _ => false,
        }
    }

    fn is_string_or_numeric_literal_like(&self, node: NodeId) -> bool {
        matches!(self.kind(node), Kind::StringLiteral | Kind::NoSubstitutionTemplateLiteral | Kind::NumericLiteral)
    }

    fn is_signed_numeric_literal(&self, node: NodeId) -> bool {
        let n = self.file.node(node);
        n.kind == Kind::PrefixUnaryExpression
            && matches!(n.op, Kind::PlusToken | Kind::MinusToken)
            && n.expression.is_some_and(|operand| self.kind(operand) == Kind::NumericLiteral)
    }

    fn has_dynamic_name(&self, node: NodeId) -> bool {
        let Some(name) = self.name_of_declaration(node) else { return false };
        if self.kind(name) != Kind::ComputedPropertyName {
            return false;
        }
        let Some(expression) = self.file.node(name).expression else { return false };
        !self.is_string_or_numeric_literal_like(expression) && !self.is_signed_numeric_literal(expression)
    }

    fn is_class_like(&self, node: NodeId) -> bool {
        matches!(self.kind(node), Kind::ClassDeclaration | Kind::ClassExpression)
    }

    fn containing_class(&self, node: NodeId) -> Option<NodeId> {
        let mut current = self.parent(node);
        while let Some(id) = current {
            if self.is_class_like(id) {
                return Some(id);
            }
            current = self.parent(id);
        }
        None
    }

    fn is_class_element(&self, node: NodeId) -> bool {
        matches!(
            self.kind(node),
            Kind::Constructor
                | Kind::PropertyDeclaration
                | Kind::MethodDeclaration
                | Kind::GetAccessor
                | Kind::SetAccessor
                | Kind::IndexSignature
                | Kind::ClassStaticBlockDeclaration
                | Kind::SemicolonClassElement
        )
    }

    fn is_static(&self, node: NodeId) -> bool {
        self.is_class_element(node) && self.has_modifier(node, Kind::StaticKeyword)
            || self.kind(node) == Kind::ClassStaticBlockDeclaration
    }

    fn is_entity_name_expression(&self, node: NodeId) -> bool {
        match self.kind(node) {
            Kind::Identifier => true,
            Kind::PropertyAccessExpression => {
                let n = self.file.node(node);
                n.name.is_some_and(|name| self.kind(name) == Kind::Identifier)
                    && n.expression.is_some_and(|expression| self.is_entity_name_expression(expression))
            }
            _ => false,
        }
    }

    fn is_identifier_name(&self, node: NodeId) -> bool {
        let Some(parent) = self.parent(node) else { return false };
        let p = self.file.node(parent);
        match p.kind {
            Kind::PropertyDeclaration
            | Kind::PropertySignature
            | Kind::MethodDeclaration
            | Kind::MethodSignature
            | Kind::GetAccessor
            | Kind::SetAccessor
            | Kind::EnumMember
            | Kind::PropertyAssignment
            | Kind::PropertyAccessExpression => p.name == Some(node),
            // slot: `children[1]` is a qualified name's right side, a binding
            // element's property name; `children[0]` an import specifier's.
            Kind::QualifiedName => p.children.get(1).copied().flatten() == Some(node),
            Kind::BindingElement => p.children.get(1).copied().flatten() == Some(node),
            Kind::ImportSpecifier => p.children.first().copied().flatten() == Some(node),
            Kind::ExportSpecifier
            | Kind::JsxAttribute
            | Kind::JsxSelfClosingElement
            | Kind::JsxOpeningElement
            | Kind::JsxClosingElement => true,
            _ => false,
        }
    }

    /// `ast.GetThisContainer(node, includeArrowFunctions: true, false)`.
    fn this_container(&self, node: NodeId) -> NodeId {
        let mut node = node;
        loop {
            let Some(parent) = self.parent(node) else { return self.file.root };
            node = parent;
            match self.kind(node) {
                Kind::ComputedPropertyName => {
                    node = self.parent(node).and_then(|p| self.parent(p)).unwrap_or(node);
                }
                Kind::Decorator => {
                    let decorated = self.parent(node);
                    if let Some(decorated) = decorated {
                        if self.kind(decorated) == Kind::Parameter
                            && self.parent(decorated).is_some_and(|p| self.is_class_element(p))
                        {
                            node = self.parent(decorated).unwrap();
                        } else if self.is_class_element(decorated) {
                            node = decorated;
                        }
                    }
                }
                Kind::ArrowFunction
                | Kind::FunctionDeclaration
                | Kind::FunctionExpression
                | Kind::ModuleDeclaration
                | Kind::ClassStaticBlockDeclaration
                | Kind::PropertyDeclaration
                | Kind::PropertySignature
                | Kind::MethodDeclaration
                | Kind::MethodSignature
                | Kind::Constructor
                | Kind::GetAccessor
                | Kind::SetAccessor
                | Kind::CallSignature
                | Kind::ConstructSignature
                | Kind::IndexSignature
                | Kind::EnumDeclaration
                | Kind::SourceFile => return node,
                _ => {}
            }
        }
    }

    fn is_in_top_level_context(&self, node: NodeId) -> bool {
        let mut node = node;
        if self.kind(node) == Kind::Identifier {
            if let Some(parent) = self.parent(node) {
                if matches!(self.kind(parent), Kind::ClassDeclaration | Kind::FunctionDeclaration)
                    && self.name_of(parent) == Some(node)
                {
                    node = parent;
                }
            }
        }
        self.kind(self.this_container(node)) == Kind::SourceFile
    }

    fn is_declaration_statement(&self, node: NodeId) -> bool {
        matches!(
            self.kind(node),
            Kind::FunctionDeclaration
                | Kind::MissingDeclaration
                | Kind::ClassDeclaration
                | Kind::InterfaceDeclaration
                | Kind::TypeAliasDeclaration
                | Kind::EnumDeclaration
                | Kind::ModuleDeclaration
                | Kind::ImportDeclaration
                | Kind::ImportEqualsDeclaration
                | Kind::ExportDeclaration
                | Kind::ExportAssignment
                | Kind::NamespaceExportDeclaration
        )
    }

    fn is_block_or_catch_scoped(&self, node: NodeId) -> bool {
        if self.combined_node_flags(node).has(NodeFlags::BlockScoped) {
            return true;
        }
        let root = self.root_declaration(node);
        self.kind(root) == Kind::VariableDeclaration
            && self.parent(root).is_some_and(|p| self.kind(p) == Kind::CatchClause)
    }

    fn is_binding_pattern(&self, node: Option<NodeId>) -> bool {
        node.is_some_and(|n| matches!(self.kind(n), Kind::ObjectBindingPattern | Kind::ArrayBindingPattern))
    }

    // --- module instance state ----------------------------------------------

    fn module_instance_state(&self, node: NodeId) -> ModuleInstanceState {
        let mut visited = HashMap::new();
        self.module_instance_state_of(node, &mut vec![], &mut visited)
    }

    fn module_instance_state_of(
        &self,
        node: NodeId,
        ancestors: &mut Vec<NodeId>,
        visited: &mut HashMap<NodeId, ModuleInstanceState>,
    ) -> ModuleInstanceState {
        match self.file.node(node).body {
            Some(body) => {
                ancestors.push(node);
                let state = self.module_instance_state_cached(body, ancestors, visited);
                ancestors.pop();
                state
            }
            None => ModuleInstanceState::Instantiated,
        }
    }

    fn module_instance_state_cached(
        &self,
        node: NodeId,
        ancestors: &mut Vec<NodeId>,
        visited: &mut HashMap<NodeId, ModuleInstanceState>,
    ) -> ModuleInstanceState {
        if let Some(&cached) = visited.get(&node) {
            return if cached != ModuleInstanceState::Unknown { cached } else { ModuleInstanceState::NonInstantiated };
        }
        visited.insert(node, ModuleInstanceState::Unknown);
        let result = self.module_instance_state_worker(node, ancestors, visited);
        visited.insert(node, result);
        result
    }

    fn module_instance_state_worker(
        &self,
        node: NodeId,
        ancestors: &mut Vec<NodeId>,
        visited: &mut HashMap<NodeId, ModuleInstanceState>,
    ) -> ModuleInstanceState {
        match self.kind(node) {
            Kind::InterfaceDeclaration | Kind::TypeAliasDeclaration => return ModuleInstanceState::NonInstantiated,
            Kind::EnumDeclaration => {
                if self.combined_has_modifier(node, Kind::ConstKeyword) {
                    return ModuleInstanceState::ConstEnumOnly;
                }
            }
            Kind::ImportDeclaration | Kind::ImportEqualsDeclaration => {
                if !self.has_modifier(node, Kind::ExportKeyword) {
                    return ModuleInstanceState::NonInstantiated;
                }
            }
            Kind::ExportDeclaration => {
                // slot: `children[0]` is the export clause, `children[1]` the module specifier.
                let n = self.file.node(node);
                let export_clause = n.children.first().copied().flatten();
                let module_specifier = n.children.get(1).copied().flatten();
                if let Some(export_clause) = export_clause {
                    if module_specifier.is_none() && self.kind(export_clause) == Kind::NamedExports {
                        let mut state = ModuleInstanceState::NonInstantiated;
                        ancestors.push(node);
                        ancestors.push(export_clause);
                        for specifier in self.statements(export_clause) {
                            let specifier_state = self.module_instance_state_for_alias_target(specifier, ancestors, visited);
                            if specifier_state > state {
                                state = specifier_state;
                            }
                            if state == ModuleInstanceState::Instantiated {
                                break;
                            }
                        }
                        ancestors.pop();
                        ancestors.pop();
                        return state;
                    }
                }
            }
            Kind::ModuleBlock => {
                let mut state = ModuleInstanceState::NonInstantiated;
                ancestors.push(node);
                for child in self.file.children(node) {
                    match self.module_instance_state_cached(child, ancestors, visited) {
                        ModuleInstanceState::NonInstantiated => {}
                        ModuleInstanceState::ConstEnumOnly => state = ModuleInstanceState::ConstEnumOnly,
                        ModuleInstanceState::Instantiated => {
                            state = ModuleInstanceState::Instantiated;
                            break;
                        }
                        ModuleInstanceState::Unknown => unreachable!(),
                    }
                }
                ancestors.pop();
                return state;
            }
            Kind::ModuleDeclaration => return self.module_instance_state_of(node, ancestors, visited),
            _ => {}
        }
        ModuleInstanceState::Instantiated
    }

    fn module_instance_state_for_alias_target(
        &self,
        node: NodeId,
        ancestors: &mut Vec<NodeId>,
        visited: &mut HashMap<NodeId, ModuleInstanceState>,
    ) -> ModuleInstanceState {
        let n = self.file.node(node);
        let Some(name) = n.children.first().copied().flatten().or(n.name) else {
            return ModuleInstanceState::Instantiated;
        };
        if self.kind(name) != Kind::Identifier {
            return ModuleInstanceState::Instantiated;
        }
        let name_text = self.node_text(name);
        // `popAncestor`: the recorded ancestors from the innermost, then the
        // real parents once they run out.
        let mut stack = ancestors.clone();
        let mut current = node;
        loop {
            let p = match stack.pop() {
                Some(p) => p,
                None => match self.parent(current) {
                    Some(p) => p,
                    None => break,
                },
            };
            current = p;
            if matches!(self.kind(p), Kind::Block | Kind::ModuleBlock | Kind::SourceFile) {
                let mut found = ModuleInstanceState::Unknown;
                let mut statement_ancestors = stack.clone();
                statement_ancestors.push(p);
                for statement in self.statements(p) {
                    if self.node_has_name(statement, &name_text) {
                        let state = self.module_instance_state_cached(statement, &mut statement_ancestors, visited);
                        if found == ModuleInstanceState::Unknown || state > found {
                            found = state;
                        }
                        if found == ModuleInstanceState::Instantiated {
                            return found;
                        }
                        if self.kind(statement) == Kind::ImportEqualsDeclaration {
                            found = ModuleInstanceState::Instantiated;
                        }
                    }
                }
                if found != ModuleInstanceState::Unknown {
                    return found;
                }
            }
        }
        ModuleInstanceState::Instantiated
    }

    fn node_has_name(&self, statement: NodeId, name: &str) -> bool {
        if let Some(declared) = self.name_of(statement) {
            return self.kind(declared) == Kind::Identifier && self.node_text(declared) == name;
        }
        if self.kind(statement) == Kind::VariableStatement {
            let Some(list) = self.file.node(statement).children.first().copied().flatten() else { return false };
            return self.statements(list).into_iter().any(|declaration| self.node_has_name(declaration, name));
        }
        false
    }

    // --- diagnostics ---------------------------------------------------------

    fn range_of_token_at(&self, pos: usize) -> (usize, usize) {
        let mut scanner = Scanner::new(self.text, self.jsx);
        scanner.reset_pos(pos);
        scanner.scan();
        (scanner.token_start(), scanner.token_end())
    }

    /// `scanner.GetErrorRangeForNode`.
    fn error_range_for_node(&self, node: NodeId) -> (usize, usize) {
        let mut error_node = Some(node);
        match self.kind(node) {
            Kind::SourceFile => {
                let pos = scanner::skip_trivia(self.text, 0);
                if pos == self.text.len() {
                    return (0, 0);
                }
                return self.range_of_token_at(pos);
            }
            Kind::FunctionDeclaration
            | Kind::MethodDeclaration
            | Kind::VariableDeclaration
            | Kind::BindingElement
            | Kind::ClassDeclaration
            | Kind::InterfaceDeclaration
            | Kind::ModuleDeclaration
            | Kind::EnumDeclaration
            | Kind::EnumMember
            | Kind::FunctionExpression
            | Kind::GetAccessor
            | Kind::SetAccessor
            | Kind::TypeAliasDeclaration
            | Kind::PropertyDeclaration
            | Kind::PropertySignature
            | Kind::NamespaceImport => error_node = self.name_of_declaration(node),
            Kind::ClassExpression => error_node = self.name_of(node),
            Kind::Constructor => {
                let mut scanner = Scanner::new(self.text, self.jsx);
                scanner.reset_pos(self.file.node(node).pos);
                scanner.scan();
                let start = scanner.token_start();
                while !matches!(scanner.token(), Kind::ConstructorKeyword | Kind::StringLiteral | Kind::EndOfFile) {
                    scanner.scan();
                }
                return (start, scanner.token_end());
            }
            _ => {}
        }
        let Some(error_node) = error_node else {
            return self.range_of_token_at(self.file.node(node).pos);
        };
        let n = self.file.node(error_node);
        let mut pos = n.pos;
        if !(n.pos == n.end && n.kind != Kind::EndOfFile) && n.kind != Kind::JsxText {
            pos = scanner::skip_trivia(self.text, pos);
        }
        (pos, n.end)
    }

    /// `rangeOfTypeParameters`: a type parameter list with its `<>`.
    fn range_of_type_parameters(&self, list: &ast::NodeList) -> (usize, usize) {
        let end = scanner::skip_trivia(self.text, list.end) + 1;
        (list.pos.saturating_sub(1), end.min(self.text.len()))
    }

    fn diagnostic(&self, (start, end): (usize, usize), message: &'static Message, args: Vec<String>) -> Diagnostic {
        Diagnostic { start, end, message, args }
    }

    fn error_on_node(&mut self, node: NodeId, message: &'static Message, args: Vec<String>) {
        let diagnostic = self.diagnostic(self.error_range_for_node(node), message, args);
        self.diagnostics.push(diagnostic);
    }

    fn error_on_first_token(&mut self, node: NodeId, message: &'static Message, args: Vec<String>) {
        let range = self.range_of_token_at(self.file.node(node).pos);
        let diagnostic = self.diagnostic(range, message, args);
        self.diagnostics.push(diagnostic);
    }

    /// `scanner.DeclarationNameToString`.
    fn declaration_name_to_string(&self, name: Option<NodeId>) -> String {
        let Some(name) = name else { return "(Missing)".to_string() };
        let n = self.file.node(name);
        if n.pos == n.end {
            return "(Missing)".to_string();
        }
        let start = scanner::skip_trivia(self.text, n.pos);
        self.text.get(start..n.end).unwrap_or_default().to_string()
    }

    // --- declarations --------------------------------------------------------

    fn declaration_name(&self, node: NodeId) -> String {
        if self.kind(node) == Kind::ExportAssignment {
            return if self.file.node(node).is_export_equals { EXPORT_EQUALS } else { DEFAULT }.to_string();
        }
        if let Some(name) = self.name_of_declaration(node) {
            if self.is_ambient_module(node) {
                if self.is_global_scope_augmentation(node) {
                    return internal("global");
                }
                return format!("\"{}\"", self.node_text(name));
            }
            return match self.kind(name) {
                Kind::PrivateIdentifier => match self.containing_class(node).and_then(|class| self.symbol_of(class)) {
                    Some(class_symbol) => format!("{INTERNAL_PREFIX}#{class_symbol}@{}", self.node_text(name)),
                    None => MISSING.to_string(),
                },
                Kind::Identifier
                | Kind::StringLiteral
                | Kind::NoSubstitutionTemplateLiteral
                | Kind::NumericLiteral
                | Kind::JsxNamespacedName => self.node_text(name),
                Kind::ComputedPropertyName => {
                    let Some(expression) = self.file.node(name).expression else { return MISSING.to_string() };
                    if self.is_string_or_numeric_literal_like(expression) {
                        self.node_text(expression)
                    } else if self.is_signed_numeric_literal(expression) {
                        let e = self.file.node(expression);
                        let operand = e.expression.map(|o| self.node_text(o)).unwrap_or_default();
                        format!("{}{operand}", scanner::token_to_string(e.op))
                    } else {
                        MISSING.to_string()
                    }
                }
                _ => MISSING.to_string(),
            };
        }
        match self.kind(node) {
            Kind::Constructor => internal("constructor"),
            Kind::FunctionType | Kind::CallSignature => internal("call"),
            Kind::ConstructorType | Kind::ConstructSignature => internal("new"),
            Kind::IndexSignature => internal("index"),
            Kind::ExportDeclaration => internal("export"),
            Kind::SourceFile | Kind::BinaryExpression => EXPORT_EQUALS.to_string(),
            _ => MISSING.to_string(),
        }
    }

    fn display_name(&self, node: NodeId) -> String {
        if let Some(name) = self.name_of(node) {
            return self.declaration_name_to_string(Some(name));
        }
        let name = self.declaration_name(node);
        if name != MISSING { name } else { "(Missing)".to_string() }
    }

    fn is_default_export(&self, node: NodeId) -> bool {
        self.has_modifier(node, Kind::DefaultKeyword)
            || self.kind(node) == Kind::ExportSpecifier
                && self.name_of(node).is_some_and(|name| {
                    matches!(self.kind(name), Kind::Identifier | Kind::StringLiteral) && self.node_text(name) == DEFAULT
                })
    }

    fn declare_symbol(&mut self, table: Option<Table>, has_parent: bool, node: NodeId, includes: u32, excludes: u32) -> SymbolId {
        let is_default_export = self.is_default_export(node);
        let name = if is_default_export && has_parent { DEFAULT.to_string() } else { self.declaration_name(node) };
        let symbol;
        match table {
            Some(table) if name != MISSING => {
                let existing = self.table(table).get(&name).copied();
                match existing {
                    None => {
                        symbol = self.new_symbol(sf::None);
                        self.table(table).insert(name, symbol);
                    }
                    Some(existing) if self.symbols[existing].flags & excludes != 0 => {
                        if self.symbols[existing].flags & sf::ReplaceableByMethod != 0 {
                            symbol = self.new_symbol(sf::None);
                            self.table(table).insert(name, symbol);
                        } else if !(includes & sf::Variable != 0 && self.symbols[existing].flags & sf::Assignment != 0
                            || includes & sf::Assignment != 0 && self.symbols[existing].flags & sf::Variable != 0)
                        {
                            self.report_conflict(existing, node, includes, is_default_export);
                            symbol = self.new_symbol(sf::None);
                        } else {
                            symbol = existing;
                        }
                    }
                    Some(existing) => symbol = existing,
                }
            }
            _ => symbol = self.new_symbol(sf::None),
        }
        self.add_declaration_to_symbol(symbol, node, includes);
        symbol
    }

    fn report_conflict(&mut self, symbol: SymbolId, node: NodeId, includes: u32, is_default_export: bool) {
        let symbol_flags = self.symbols[symbol].flags;
        let mut message = if symbol_flags & sf::BlockScopedVariable != 0 {
            diagnostics::Cannot_redeclare_block_scoped_variable_0
        } else {
            diagnostics::Duplicate_identifier_0
        };
        let mut message_needs_name = true;
        if symbol_flags & sf::Enum != 0 || includes & sf::Enum != 0 {
            message = diagnostics::Enum_declarations_can_only_merge_with_namespace_or_other_enum_declarations;
            message_needs_name = false;
        }
        let declarations = self.symbols[symbol].declarations.clone();
        if !declarations.is_empty()
            && (is_default_export
                || self.kind(node) == Kind::ExportAssignment && !self.file.node(node).is_export_equals)
        {
            message = diagnostics::A_module_cannot_have_multiple_default_exports;
            message_needs_name = false;
        }
        let declaration_name = self.name_of_declaration(node).unwrap_or(node);
        let args = |binder: &Self, declaration: NodeId| {
            if message_needs_name { vec![binder.display_name(declaration)] } else { Vec::new() }
        };
        let diagnostic = self.diagnostic(self.error_range_for_node(declaration_name), message, args(self, node));
        for declaration in declarations {
            let decl = self.name_of_declaration(declaration).unwrap_or(declaration);
            let d = self.diagnostic(self.error_range_for_node(decl), message, args(self, declaration));
            self.diagnostics.push(d);
        }
        self.diagnostics.push(diagnostic);
        if symbol_flags & sf::Accessor != 0 && symbol_flags & sf::Accessor != includes & sf::Accessor {
            self.symbols[symbol].flags |= sf::Accessor;
        }
    }

    fn add_declaration_to_symbol(&mut self, symbol: SymbolId, node: NodeId, flags: u32) {
        // `SetValueDeclaration`: any other kind of value declaration takes
        // precedence over a namespace.
        let replaces_value_declaration = flags & sf::Value != 0
            && self.symbols[symbol].value_declaration.is_none_or(|existing| {
                self.kind(existing) == Kind::ModuleDeclaration && self.kind(node) != Kind::ModuleDeclaration
            });
        let s = &mut self.symbols[symbol];
        s.flags |= flags;
        if !s.declarations.contains(&node) {
            s.declarations.push(node);
        }
        if replaces_value_declaration {
            s.value_declaration = Some(node);
        }
        self.node_symbol.insert(node, symbol);
    }

    fn declare_module_member(&mut self, node: NodeId, flags: u32, excludes: u32) -> SymbolId {
        let container = self.container;
        let has_export_modifier = self.combined_has_modifier(node, Kind::ExportKeyword);
        if flags & sf::Alias != 0 {
            if self.kind(node) == Kind::ExportSpecifier
                || self.kind(node) == Kind::ImportEqualsDeclaration && has_export_modifier
            {
                let exports = self.exports_of(container);
                return self.declare_symbol(exports, true, node, flags, excludes);
            }
            return self.declare_symbol(Some(Table::Locals(container)), false, node, flags, excludes);
        }
        if !self.is_ambient_module(node) && (has_export_modifier || self.export_context.contains(&container)) {
            if self.has_modifier(node, Kind::DefaultKeyword) && self.declaration_name(node) == MISSING {
                let exports = self.exports_of(container);
                return self.declare_symbol(exports, true, node, flags, excludes);
            }
            let export_kind = if flags & sf::Value != 0 { sf::ExportValue } else { sf::None };
            let local = self.declare_symbol(Some(Table::Locals(container)), false, node, export_kind, excludes);
            let exports = self.exports_of(container);
            let export = self.declare_symbol(exports, true, node, flags, excludes);
            self.symbols[local].export_symbol = Some(export);
            self.local_symbol.insert(node, local);
            return local;
        }
        self.declare_symbol(Some(Table::Locals(container)), false, node, flags, excludes)
    }

    fn declare_class_member(&mut self, node: NodeId, flags: u32, excludes: u32) -> SymbolId {
        let table = if self.is_static(node) { self.exports_of(self.container) } else { self.members_of(self.container) };
        self.declare_symbol(table, true, node, flags, excludes)
    }

    fn declare_source_file_member(&mut self, node: NodeId, flags: u32, excludes: u32) -> SymbolId {
        if self.file.external_module {
            return self.declare_module_member(node, flags, excludes);
        }
        self.declare_symbol(Some(Table::Locals(self.file.root)), false, node, flags, excludes)
    }

    fn declare_symbol_and_add_to_symbol_table(&mut self, node: NodeId, flags: u32, excludes: u32) -> SymbolId {
        let container = self.container;
        match self.kind(container) {
            Kind::ModuleDeclaration => self.declare_module_member(node, flags, excludes),
            Kind::SourceFile => self.declare_source_file_member(node, flags, excludes),
            Kind::ClassExpression | Kind::ClassDeclaration => self.declare_class_member(node, flags, excludes),
            Kind::EnumDeclaration => {
                let exports = self.exports_of(container);
                self.declare_symbol(exports, true, node, flags, excludes)
            }
            Kind::TypeLiteral | Kind::ObjectLiteralExpression | Kind::InterfaceDeclaration | Kind::JsxAttributes => {
                let members = self.members_of(container);
                self.declare_symbol(members, true, node, flags, excludes)
            }
            _ => self.declare_symbol(Some(Table::Locals(container)), false, node, flags, excludes),
        }
    }

    fn bind_anonymous_declaration(&mut self, node: NodeId, flags: u32) {
        let symbol = self.new_symbol(flags);
        self.add_declaration_to_symbol(symbol, node, flags);
    }

    fn bind_block_scoped_declaration(&mut self, node: NodeId, flags: u32, excludes: u32) {
        let block_scope_container = self.block_scope_container;
        match self.kind(block_scope_container) {
            Kind::ModuleDeclaration => {
                self.declare_module_member(node, flags, excludes);
            }
            Kind::SourceFile if self.file.external_module => {
                self.declare_module_member(node, flags, excludes);
            }
            _ => {
                self.declare_symbol(Some(Table::Locals(block_scope_container)), false, node, flags, excludes);
            }
        }
    }

    // --- global contributions ------------------------------------------------

    /// What the checker's `initializeChecker` merges into the global symbol
    /// table from this file: a script's locals, or a module's augmentations
    /// (top-level `declare global` and `declare module "…"` blocks).
    fn globals(&self) -> FileGlobals {
        let mut out = FileGlobals::default();
        let mut ids = HashMap::new();
        if !self.file.external_module {
            out.is_script = true;
            out.locals = self.export_table(Table::Locals(self.file.root), &mut out, &mut ids);
            return out;
        }
        let mut global_augmentations = Vec::new();
        let mut module_augmentations: Vec<(String, SymbolId)> = Vec::new();
        for statement in self.statements(self.file.root) {
            if !self.is_ambient_module(statement)
                || !(self.has_modifier(statement, Kind::DeclareKeyword) || self.file.is_declaration_file)
            {
                continue;
            }
            let Some(symbol) = self.symbol_of(statement) else { continue };
            if self.is_global_scope_augmentation(statement) {
                if !global_augmentations.contains(&symbol) {
                    global_augmentations.push(symbol);
                }
            } else if !module_augmentations.iter().any(|&(_, s)| s == symbol) {
                let name = self.name_of(statement).map(|n| self.node_text(n)).unwrap_or_default();
                module_augmentations.push((name, symbol));
            }
        }
        for symbol in global_augmentations {
            let exports = self.export_table(Table::Exports(symbol), &mut out, &mut ids);
            out.augmentations.push(exports);
        }
        for (module_name, symbol) in module_augmentations {
            let name = format!("\"{module_name}\"");
            let id = self.export_symbol(&name, symbol, &mut out, &mut ids);
            out.module_augmentations.push((module_name, id));
        }
        out
    }

    fn export_table(&self, table: Table, out: &mut FileGlobals, ids: &mut HashMap<SymbolId, u32>) -> Vec<u32> {
        let Some(entries) = self.tables.get(&table) else { return Vec::new() };
        let mut entries: Vec<(&String, SymbolId)> = entries.iter().map(|(name, &symbol)| (name, symbol)).collect();
        entries.sort_by_key(|&(name, symbol)| {
            let first = self.symbols[symbol].declarations.first().map_or(usize::MAX, |&d| self.file.node(d).pos);
            (first, name.clone())
        });
        entries.into_iter().map(|(name, symbol)| self.export_symbol(name, symbol, out, ids)).collect()
    }

    fn export_symbol(&self, name: &str, symbol: SymbolId, out: &mut FileGlobals, ids: &mut HashMap<SymbolId, u32>) -> u32 {
        if let Some(&id) = ids.get(&symbol) {
            return id;
        }
        let id = out.symbols.len() as u32;
        out.symbols.push(GlobalSymbol::default());
        ids.insert(symbol, id);
        let declarations = self.symbols[symbol]
            .declarations
            .iter()
            .map(|&declaration| GlobalDeclaration {
                name_range: self.error_range_for_node(self.name_of_declaration(declaration).unwrap_or(declaration)),
                node_range: self.error_range_for_node(declaration),
                is_type_declaration: self.is_type_declaration(declaration),
                type_parameters_range: self
                    .file
                    .node(declaration)
                    .type_parameters
                    .as_ref()
                    .filter(|list| !list.nodes.is_empty())
                    .map(|list| self.range_of_type_parameters(list)),
            })
            .collect();
        let members = self
            .tables
            .contains_key(&Table::Members(symbol))
            .then(|| self.export_table(Table::Members(symbol), out, ids));
        let exports = self
            .tables
            .contains_key(&Table::Exports(symbol))
            .then(|| self.export_table(Table::Exports(symbol), out, ids));
        out.symbols[id as usize] =
            GlobalSymbol { name: name.to_string(), flags: self.symbols[symbol].flags, declarations, members, exports };
        id
    }

    /// `ast.IsTypeDeclaration`.
    fn is_type_declaration(&self, node: NodeId) -> bool {
        match self.kind(node) {
            Kind::TypeParameter
            | Kind::ClassDeclaration
            | Kind::InterfaceDeclaration
            | Kind::TypeAliasDeclaration
            | Kind::EnumDeclaration => true,
            Kind::ImportClause => self.file.node(node).is_type_only,
            Kind::ImportSpecifier | Kind::ExportSpecifier => self
                .parent(node)
                .and_then(|p| self.parent(p))
                .is_some_and(|clause| self.file.node(clause).is_type_only),
            _ => false,
        }
    }

    // --- binding -------------------------------------------------------------

    fn bind(&mut self, node: Option<NodeId>) {
        let Some(node) = node else { return };
        match self.kind(node) {
            Kind::Identifier => self.check_contextual_identifier(node),
            Kind::PrivateIdentifier => self.check_private_identifier(node),
            Kind::BinaryExpression => self.check_strict_mode_binary_expression(node),
            Kind::CatchClause => self.check_strict_mode_catch_clause(node),
            Kind::DeleteExpression => self.check_strict_mode_delete_expression(node),
            Kind::PostfixUnaryExpression => {
                let operand = self.file.node(node).expression;
                self.check_strict_mode_eval_or_arguments(node, operand);
            }
            Kind::PrefixUnaryExpression => {
                let n = self.file.node(node);
                if matches!(n.op, Kind::PlusPlusToken | Kind::MinusMinusToken) {
                    let operand = n.expression;
                    self.check_strict_mode_eval_or_arguments(node, operand);
                }
            }
            Kind::WithStatement => {
                self.error_on_first_token(node, diagnostics::X_with_statements_are_not_allowed_in_strict_mode, vec![]);
            }
            Kind::LabeledStatement => self.check_strict_mode_labeled_statement(node),
            Kind::TypeParameter => self.bind_type_parameter(node),
            Kind::Parameter => self.bind_parameter(node),
            Kind::VariableDeclaration | Kind::BindingElement => self.bind_variable_declaration_or_binding_element(node),
            Kind::PropertyDeclaration | Kind::PropertySignature => self.bind_property_worker(node),
            Kind::PropertyAssignment | Kind::ShorthandPropertyAssignment => {
                self.bind_property_or_method_or_accessor(node, sf::Property, sf::PropertyExcludes);
            }
            Kind::EnumMember => self.bind_property_or_method_or_accessor(node, sf::EnumMember, sf::EnumMemberExcludes),
            Kind::CallSignature | Kind::ConstructSignature | Kind::IndexSignature => {
                self.declare_symbol_and_add_to_symbol_table(node, sf::Signature, sf::None);
            }
            Kind::MethodDeclaration | Kind::MethodSignature => {
                let is_object_literal_method = self.kind(node) == Kind::MethodDeclaration
                    && self.parent(node).is_some_and(|p| self.kind(p) == Kind::ObjectLiteralExpression);
                let excludes = if is_object_literal_method { sf::Value } else { sf::MethodExcludes };
                self.bind_property_or_method_or_accessor(node, sf::Method | self.optional_flag(node), excludes);
            }
            Kind::FunctionDeclaration => {
                if !self.flags(node).has(NodeFlags::Ambient) {
                    let name = self.name_of(node);
                    self.check_strict_mode_eval_or_arguments(node, name);
                }
                self.bind_block_scoped_declaration(node, sf::Function, sf::FunctionExcludes);
            }
            Kind::Constructor => {
                self.declare_symbol_and_add_to_symbol_table(node, sf::Constructor, sf::None);
            }
            Kind::GetAccessor => self.bind_property_or_method_or_accessor(node, sf::GetAccessor, sf::GetAccessorExcludes),
            Kind::SetAccessor => self.bind_property_or_method_or_accessor(node, sf::SetAccessor, sf::SetAccessorExcludes),
            Kind::FunctionType | Kind::ConstructorType => self.bind_anonymous_declaration(node, sf::Signature),
            Kind::TypeLiteral | Kind::MappedType => self.bind_anonymous_declaration(node, sf::TypeLiteral),
            Kind::ObjectLiteralExpression | Kind::JsxAttributes => self.bind_anonymous_declaration(node, sf::ObjectLiteral),
            Kind::FunctionExpression | Kind::ArrowFunction => {
                if self.kind(node) == Kind::FunctionExpression && self.name_of(node).is_some() {
                    if !self.flags(node).has(NodeFlags::Ambient) {
                        let name = self.name_of(node);
                        self.check_strict_mode_eval_or_arguments(node, name);
                    }
                }
                self.bind_anonymous_declaration(node, sf::Function);
            }
            Kind::ClassDeclaration | Kind::ClassExpression => self.bind_class_like_declaration(node),
            Kind::InterfaceDeclaration => {
                self.bind_block_scoped_declaration(node, sf::Interface, sf::InterfaceExcludes);
            }
            Kind::TypeAliasDeclaration => {
                self.bind_block_scoped_declaration(node, sf::TypeAlias, sf::TypeAliasExcludes);
            }
            Kind::EnumDeclaration => {
                if self.combined_has_modifier(node, Kind::ConstKeyword) {
                    self.bind_block_scoped_declaration(node, sf::ConstEnum, sf::ConstEnumExcludes);
                } else {
                    self.bind_block_scoped_declaration(node, sf::RegularEnum, sf::RegularEnumExcludes);
                }
            }
            Kind::ModuleDeclaration => self.bind_module_declaration(node),
            Kind::ImportEqualsDeclaration | Kind::NamespaceImport | Kind::ImportSpecifier | Kind::ExportSpecifier => {
                self.declare_symbol_and_add_to_symbol_table(node, sf::Alias, sf::AliasExcludes);
            }
            Kind::NamespaceExportDeclaration => self.bind_namespace_export_declaration(node),
            Kind::ImportClause => {
                if self.name_of(node).is_some() {
                    self.declare_symbol_and_add_to_symbol_table(node, sf::Alias, sf::AliasExcludes);
                }
            }
            Kind::ExportDeclaration => self.bind_export_declaration(node),
            Kind::ExportAssignment => self.bind_export_assignment(node),
            Kind::SourceFile => self.bind_source_file_if_external_module(),
            Kind::JsxAttribute => {
                self.declare_symbol_and_add_to_symbol_table(node, sf::Property, sf::PropertyExcludes);
            }
            _ => {}
        }
        if self.kind(node) > Kind::LastToken {
            let container_flags = self.container_flags(node);
            if container_flags == cf::None {
                self.bind_children(node);
            } else {
                self.bind_container(node, container_flags);
            }
        }
    }

    fn optional_flag(&self, node: NodeId) -> u32 {
        match self.file.node(node).question_token {
            Some(token) if self.kind(token) == Kind::QuestionToken => sf::Optional,
            _ => sf::None,
        }
    }

    fn bind_property_worker(&mut self, node: NodeId) {
        let is_auto_accessor =
            self.kind(node) == Kind::PropertyDeclaration && self.has_modifier(node, Kind::AccessorKeyword);
        let includes = if is_auto_accessor { sf::Accessor } else { sf::Property };
        let excludes = if is_auto_accessor { sf::AccessorExcludes } else { sf::PropertyExcludes };
        self.bind_property_or_method_or_accessor(node, includes | self.optional_flag(node), excludes);
    }

    fn bind_property_or_method_or_accessor(&mut self, node: NodeId, flags: u32, excludes: u32) {
        if self.has_dynamic_name(node) {
            self.bind_anonymous_declaration(node, flags);
        } else {
            self.declare_symbol_and_add_to_symbol_table(node, flags, excludes);
        }
    }

    fn bind_source_file_if_external_module(&mut self) {
        let root = self.file.root;
        self.set_export_context_flag(root);
        if self.file.external_module {
            self.bind_anonymous_declaration(root, sf::ValueModule);
        }
    }

    fn set_export_context_flag(&mut self, node: NodeId) {
        let statements = match self.kind(node) {
            Kind::SourceFile => self.statements(node),
            Kind::ModuleDeclaration => match self.file.node(node).body {
                Some(body) if self.kind(body) == Kind::ModuleBlock => self.statements(body),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        let has_export_declarations = statements
            .iter()
            .any(|&s| matches!(self.kind(s), Kind::ExportDeclaration | Kind::ExportAssignment));
        self.export_context.retain(|&n| n != node);
        if self.flags(node).has(NodeFlags::Ambient) && !has_export_declarations {
            self.export_context.push(node);
        }
    }

    fn bind_module_declaration(&mut self, node: NodeId) {
        self.set_export_context_flag(node);
        if self.is_ambient_module(node) {
            if self.has_modifier(node, Kind::ExportKeyword) {
                self.error_on_first_token(
                    node,
                    diagnostics::X_export_modifier_cannot_be_applied_to_ambient_modules_and_module_augmentations_since_they_are_always_visible,
                    vec![],
                );
            }
            if self.is_module_augmentation_external(node) {
                self.declare_module_symbol(node);
            } else {
                self.declare_symbol_and_add_to_symbol_table(node, sf::ValueModule, sf::ValueModuleExcludes);
                if let Some(name) = self.name_of(node) {
                    if self.kind(name) == Kind::StringLiteral {
                        let text = self.node_text(name);
                        if let Some(star) = text.find('*') {
                            if text[star + 1..].contains('*') {
                                self.error_on_first_token(
                                    name,
                                    diagnostics::Pattern_0_can_have_at_most_one_Asterisk_character,
                                    vec![text],
                                );
                            }
                        }
                    }
                }
            }
        } else {
            self.declare_module_symbol(node);
        }
    }

    fn declare_module_symbol(&mut self, node: NodeId) {
        let instantiated = self.module_instance_state(node) != ModuleInstanceState::NonInstantiated;
        let (flags, excludes) = if instantiated {
            (sf::ValueModule, sf::ValueModuleExcludes)
        } else {
            (sf::NamespaceModule, sf::NamespaceModuleExcludes)
        };
        self.declare_symbol_and_add_to_symbol_table(node, flags, excludes);
    }

    fn bind_namespace_export_declaration(&mut self, node: NodeId) {
        if self.file.node(node).modifiers.is_some() {
            self.error_on_node(node, diagnostics::Modifiers_cannot_appear_here, vec![]);
        }
        let parent_is_source_file = self.parent(node).is_some_and(|p| self.kind(p) == Kind::SourceFile);
        if !parent_is_source_file {
            self.error_on_node(node, diagnostics::Global_module_exports_may_only_appear_at_top_level, vec![]);
        } else if !self.file.external_module {
            self.error_on_node(node, diagnostics::Global_module_exports_may_only_appear_in_module_files, vec![]);
        } else if !self.file.is_declaration_file {
            self.error_on_node(node, diagnostics::Global_module_exports_may_only_appear_in_declaration_files, vec![]);
        }
        // The global exports table is merged across files by the checker.
    }

    fn bind_export_declaration(&mut self, node: NodeId) {
        let container = self.container;
        let Some(container_symbol) = self.symbol_of(container) else {
            self.bind_anonymous_declaration(node, sf::ExportStar);
            return;
        };
        let export_clause = self.file.node(node).children.first().copied().flatten();
        match export_clause {
            None => {
                self.declare_symbol(Some(Table::Exports(container_symbol)), true, node, sf::ExportStar, sf::None);
            }
            Some(clause) if self.kind(clause) == Kind::NamespaceExport => {
                self.declare_symbol(Some(Table::Exports(container_symbol)), true, clause, sf::Alias, sf::AliasExcludes);
            }
            Some(_) => {}
        }
    }

    fn bind_export_assignment(&mut self, node: NodeId) {
        let container = self.container;
        match self.symbol_of(container) {
            None => self.bind_anonymous_declaration(node, sf::Value),
            Some(container_symbol) => {
                let expression = self.file.node(node).expression;
                let is_alias = expression
                    .is_some_and(|e| self.is_entity_name_expression(e) || self.kind(e) == Kind::ClassExpression);
                let flags = if is_alias { sf::Alias } else { sf::Property };
                self.declare_symbol(Some(Table::Exports(container_symbol)), true, node, flags, sf::All);
            }
        }
    }

    fn bind_class_like_declaration(&mut self, node: NodeId) {
        match self.kind(node) {
            Kind::ClassDeclaration => self.bind_block_scoped_declaration(node, sf::Class, sf::ClassExcludes),
            _ => self.bind_anonymous_declaration(node, sf::Class),
        }
        let Some(symbol) = self.symbol_of(node) else { return };
        let exports = Table::Exports(symbol);
        if let Some(&existing) = self.table(exports).get("prototype") {
            if let Some(&declaration) = self.symbols[existing].declarations.first() {
                self.error_on_node(declaration, diagnostics::Duplicate_identifier_0, vec!["prototype".to_string()]);
            }
        }
        let prototype = self.new_symbol(sf::Property | sf::Prototype);
        self.table(exports).insert("prototype".to_string(), prototype);
    }

    fn bind_variable_declaration_or_binding_element(&mut self, node: NodeId) {
        let name = self.name_of(node);
        self.check_strict_mode_eval_or_arguments(node, name);
        if name.is_some() && !self.is_binding_pattern(name) {
            if self.is_block_or_catch_scoped(node) {
                self.bind_block_scoped_declaration(node, sf::BlockScopedVariable, sf::BlockScopedVariableExcludes);
            } else if self.kind(self.root_declaration(node)) == Kind::Parameter {
                self.declare_symbol_and_add_to_symbol_table(node, sf::FunctionScopedVariable, sf::ParameterExcludes);
            } else {
                self.declare_symbol_and_add_to_symbol_table(
                    node,
                    sf::FunctionScopedVariable,
                    sf::FunctionScopedVariableExcludes,
                );
            }
        }
    }

    fn bind_parameter(&mut self, node: NodeId) {
        let name = self.name_of(node);
        if !self.flags(node).has(NodeFlags::Ambient) {
            self.check_strict_mode_eval_or_arguments(node, name);
        }
        if self.is_binding_pattern(name) {
            self.bind_anonymous_declaration(node, sf::FunctionScopedVariable);
        } else {
            self.declare_symbol_and_add_to_symbol_table(node, sf::FunctionScopedVariable, sf::ParameterExcludes);
        }
        let parent = self.parent(node);
        let is_parameter_property = parent.is_some_and(|p| self.kind(p) == Kind::Constructor)
            && self.file.node(node).modifier_nodes().iter().any(|&m| {
                matches!(
                    self.kind(m),
                    Kind::PublicKeyword
                        | Kind::PrivateKeyword
                        | Kind::ProtectedKeyword
                        | Kind::ReadonlyKeyword
                        | Kind::OverrideKeyword
                )
            });
        if is_parameter_property {
            let class_declaration = parent.and_then(|p| self.parent(p));
            let members = class_declaration.and_then(|class| self.members_of(class));
            let flags = sf::Property
                | if self.file.node(node).question_token.is_some() { sf::Optional } else { sf::None };
            self.declare_symbol(members, true, node, flags, sf::PropertyExcludes);
        }
    }

    fn bind_type_parameter(&mut self, node: NodeId) {
        let parent = self.parent(node);
        if parent.is_some_and(|p| self.kind(p) == Kind::InferType) {
            match self.infer_type_container(parent.unwrap()) {
                Some(container) => {
                    self.declare_symbol(
                        Some(Table::Locals(container)),
                        false,
                        node,
                        sf::TypeParameter,
                        sf::TypeParameterExcludes,
                    );
                }
                None => self.bind_anonymous_declaration(node, sf::TypeParameter),
            }
        } else {
            self.declare_symbol_and_add_to_symbol_table(node, sf::TypeParameter, sf::TypeParameterExcludes);
        }
    }

    fn infer_type_container(&self, node: NodeId) -> Option<NodeId> {
        let mut current = Some(node);
        while let Some(n) = current {
            let parent = self.parent(n)?;
            // slot: a conditional type's `children[1]` is its extends type.
            if self.kind(parent) == Kind::ConditionalType
                && self.file.node(parent).children.get(1).copied().flatten() == Some(n)
            {
                return Some(parent);
            }
            current = Some(parent);
        }
        None
    }

    // --- strict mode ---------------------------------------------------------

    fn check_contextual_identifier(&mut self, node: NodeId) {
        let flags = self.flags(node);
        if flags.has(NodeFlags::Ambient) || flags.has(NodeFlags::JSDoc) || self.is_identifier_name(node) {
            return;
        }
        let original_keyword_kind = scanner::get_identifier_token(&self.node_text(node));
        if original_keyword_kind == Kind::Identifier {
            return;
        }
        let name = self.declaration_name_to_string(Some(node));
        if (Kind::FirstFutureReservedWord..=Kind::LastFutureReservedWord).contains(&original_keyword_kind) {
            let message = self.strict_mode_identifier_message(node);
            self.error_on_node(node, message, vec![name]);
        } else if original_keyword_kind == Kind::AwaitKeyword {
            if self.file.external_module && self.is_in_top_level_context(node) {
                self.error_on_node(
                    node,
                    diagnostics::Identifier_expected_0_is_a_reserved_word_at_the_top_level_of_a_module,
                    vec![name],
                );
            } else if flags.has(NodeFlags::AwaitContext) {
                self.error_on_node(
                    node,
                    diagnostics::Identifier_expected_0_is_a_reserved_word_that_cannot_be_used_here,
                    vec![name],
                );
            }
        } else if original_keyword_kind == Kind::YieldKeyword && flags.has(NodeFlags::YieldContext) {
            self.error_on_node(
                node,
                diagnostics::Identifier_expected_0_is_a_reserved_word_that_cannot_be_used_here,
                vec![name],
            );
        }
    }

    fn check_private_identifier(&mut self, node: NodeId) {
        if self.node_text(node) == "#constructor" {
            let name = self.declaration_name_to_string(Some(node));
            self.error_on_node(node, diagnostics::X_constructor_is_a_reserved_word, vec![name]);
        }
    }

    fn strict_mode_identifier_message(&self, node: NodeId) -> &'static Message {
        if self.containing_class(node).is_some() {
            diagnostics::Identifier_expected_0_is_a_reserved_word_in_strict_mode_Class_definitions_are_automatically_in_strict_mode
        } else if self.file.external_module {
            diagnostics::Identifier_expected_0_is_a_reserved_word_in_strict_mode_Modules_are_automatically_in_strict_mode
        } else {
            diagnostics::Identifier_expected_0_is_a_reserved_word_in_strict_mode
        }
    }

    fn check_strict_mode_binary_expression(&mut self, node: NodeId) {
        let n = self.file.node(node);
        // slot: a binary expression's `children[0]` is its left operand.
        let Some(left) = n.children.first().copied().flatten() else { return };
        if ast::is_left_hand_side_expression_kind(self.kind(left)) && ast::is_assignment_operator(n.op) {
            self.check_strict_mode_eval_or_arguments(node, Some(left));
        }
    }

    fn check_strict_mode_catch_clause(&mut self, node: NodeId) {
        // slot: a catch clause's `children[0]` is its variable declaration.
        if let Some(declaration) = self.file.node(node).children.first().copied().flatten() {
            let name = self.name_of(declaration);
            self.check_strict_mode_eval_or_arguments(node, name);
        }
    }

    fn check_strict_mode_delete_expression(&mut self, node: NodeId) {
        if let Some(expression) = self.file.node(node).expression {
            if self.kind(expression) == Kind::Identifier {
                self.error_on_node(expression, diagnostics::X_delete_cannot_be_called_on_an_identifier_in_strict_mode, vec![]);
            }
        }
    }

    fn check_strict_mode_labeled_statement(&mut self, node: NodeId) {
        // slot: a labeled statement's `children` are its label and statement.
        let n = self.file.node(node);
        let (Some(label), Some(statement)) = (n.children.first().copied().flatten(), n.children.get(1).copied().flatten())
        else {
            return;
        };
        if self.is_declaration_statement(statement) || self.kind(statement) == Kind::VariableStatement {
            self.error_on_first_token(label, diagnostics::A_label_is_not_allowed_here, vec![]);
        }
    }

    fn check_strict_mode_eval_or_arguments(&mut self, context_node: NodeId, name: Option<NodeId>) {
        let Some(name) = name else { return };
        if self.kind(name) != Kind::Identifier {
            return;
        }
        let text = self.node_text(name);
        if text != "eval" && text != "arguments" {
            return;
        }
        let message = if self.containing_class(context_node).is_some() {
            diagnostics::Code_contained_in_a_class_is_evaluated_in_JavaScript_s_strict_mode_which_does_not_allow_this_use_of_0_For_more_information_see_https_Colon_Slash_Slashdeveloper_mozilla_org_Slashen_US_Slashdocs_SlashWeb_SlashJavaScript_SlashReference_SlashStrict_mode
        } else if self.file.external_module {
            diagnostics::Invalid_use_of_0_Modules_are_automatically_in_strict_mode
        } else {
            diagnostics::Invalid_use_of_0_in_strict_mode
        };
        self.error_on_node(name, message, vec![text]);
    }

    // --- containers ----------------------------------------------------------

    fn container_flags(&self, node: NodeId) -> u32 {
        match self.kind(node) {
            Kind::ClassExpression
            | Kind::ClassDeclaration
            | Kind::EnumDeclaration
            | Kind::ObjectLiteralExpression
            | Kind::TypeLiteral
            | Kind::JsxAttributes => cf::IsContainer,
            Kind::InterfaceDeclaration => cf::IsContainer | cf::IsInterface,
            Kind::ModuleDeclaration | Kind::TypeAliasDeclaration | Kind::MappedType | Kind::IndexSignature => {
                cf::IsContainer | cf::HasLocals
            }
            Kind::SourceFile => cf::IsContainer | cf::IsControlFlowContainer | cf::HasLocals,
            Kind::GetAccessor
            | Kind::SetAccessor
            | Kind::MethodDeclaration
            | Kind::Constructor
            | Kind::FunctionDeclaration
            | Kind::ClassStaticBlockDeclaration
            | Kind::MethodSignature
            | Kind::CallSignature
            | Kind::FunctionType
            | Kind::ConstructSignature
            | Kind::ConstructorType
            | Kind::FunctionExpression
            | Kind::ArrowFunction => cf::IsContainer | cf::IsControlFlowContainer | cf::HasLocals | cf::IsFunctionLike,
            Kind::ModuleBlock => cf::IsControlFlowContainer,
            Kind::PropertyDeclaration => {
                if self.file.node(node).initializer.is_some() {
                    cf::IsControlFlowContainer
                } else {
                    cf::None
                }
            }
            Kind::CatchClause | Kind::ForStatement | Kind::ForInStatement | Kind::ForOfStatement | Kind::CaseBlock => {
                cf::IsBlockScopedContainer | cf::HasLocals
            }
            Kind::Block => {
                let parent_is_function = self.parent(node).is_some_and(|p| {
                    ast::is_function_like_kind(self.kind(p)) || self.kind(p) == Kind::ClassStaticBlockDeclaration
                });
                if parent_is_function { cf::None } else { cf::IsBlockScopedContainer | cf::HasLocals }
            }
            _ => cf::None,
        }
    }

    fn bind_container(&mut self, node: NodeId, container_flags: u32) {
        let save_container = self.container;
        let save_block_scope_container = self.block_scope_container;
        if container_flags & cf::IsContainer != 0 {
            self.container = node;
            self.block_scope_container = node;
        } else if container_flags & cf::IsBlockScopedContainer != 0 {
            self.block_scope_container = node;
        }
        self.bind_children(node);
        self.container = save_container;
        self.block_scope_container = save_block_scope_container;
    }

    fn bind_children(&mut self, node: NodeId) {
        match self.kind(node) {
            Kind::SourceFile | Kind::Block | Kind::ModuleBlock => {
                let statements = self.statements(node);
                for &statement in &statements {
                    if self.kind(statement) == Kind::FunctionDeclaration {
                        self.bind(Some(statement));
                    }
                }
                for &statement in &statements {
                    if self.kind(statement) != Kind::FunctionDeclaration {
                        self.bind(Some(statement));
                    }
                }
            }
            _ => {
                for child in self.file.children(node) {
                    self.bind(Some(child));
                }
            }
        }
    }
}
