#![allow(unused_imports)]

use crate::ast::{self, Node, NodeId, NodeList, OperatorPrecedence};
use crate::flags::{NodeFlags, TokenFlags};
use crate::kind::Kind;
use crate::messages as diagnostics;
use crate::scanner::{self, token_is_identifier_or_keyword, token_to_string};
use crate::Message;

use super::{PC, ParseFlags, Parser, Tristate};

impl<'a> Parser<'a> {
    pub(crate) fn parse_statement(&mut self) -> NodeId {
        match self.token {
            Kind::SemicolonToken => return self.parse_empty_statement(),
            Kind::OpenBraceToken => return self.parse_block(false, None),
            Kind::VarKeyword => {
                let pos = self.node_pos();
                return self.parse_variable_statement(pos, None);
            }
            Kind::LetKeyword => {
                if self.is_let_declaration() {
                    let pos = self.node_pos();
                    return self.parse_variable_statement(pos, None);
                }
            }
            Kind::AwaitKeyword => {
                if self.is_await_using_declaration() {
                    let pos = self.node_pos();
                    return self.parse_variable_statement(pos, None);
                }
            }
            Kind::UsingKeyword => {
                if self.is_using_declaration() {
                    let pos = self.node_pos();
                    return self.parse_variable_statement(pos, None);
                }
            }
            Kind::FunctionKeyword => {
                let pos = self.node_pos();
                return self.parse_function_declaration(pos, None);
            }
            Kind::ClassKeyword => {
                let pos = self.node_pos();
                return self.parse_class_declaration(pos, None);
            }
            Kind::IfKeyword => return self.parse_if_statement(),
            Kind::DoKeyword => return self.parse_do_statement(),
            Kind::WhileKeyword => return self.parse_while_statement(),
            Kind::ForKeyword => return self.parse_for_or_for_in_or_for_of_statement(),
            Kind::ContinueKeyword => return self.parse_continue_statement(),
            Kind::BreakKeyword => return self.parse_break_statement(),
            Kind::ReturnKeyword => return self.parse_return_statement(),
            Kind::WithKeyword => return self.parse_with_statement(),
            Kind::SwitchKeyword => return self.parse_switch_statement(),
            Kind::ThrowKeyword => return self.parse_throw_statement(),
            Kind::TryKeyword | Kind::CatchKeyword | Kind::FinallyKeyword => return self.parse_try_statement(),
            Kind::DebuggerKeyword => return self.parse_debugger_statement(),
            Kind::AtToken => return self.parse_declaration(),
            Kind::AsyncKeyword
            | Kind::InterfaceKeyword
            | Kind::TypeKeyword
            | Kind::ModuleKeyword
            | Kind::NamespaceKeyword
            | Kind::DeclareKeyword
            | Kind::ConstKeyword
            | Kind::EnumKeyword
            | Kind::ExportKeyword
            | Kind::ImportKeyword
            | Kind::PrivateKeyword
            | Kind::ProtectedKeyword
            | Kind::PublicKeyword
            | Kind::AbstractKeyword
            | Kind::AccessorKeyword
            | Kind::StaticKeyword
            | Kind::ReadonlyKeyword
            | Kind::GlobalKeyword => {
                if self.is_start_of_declaration() {
                    return self.parse_declaration();
                }
            }
            _ => {}
        }
        self.parse_expression_or_labeled_statement()
    }

    pub(crate) fn parse_declaration(&mut self) -> NodeId {
        let pos = self.node_pos();
        let modifiers = self.parse_modifiers_ex(true, false, false);
        let is_ambient = self.modifier_list_has(&modifiers, Kind::DeclareKeyword);
        if is_ambient {
            if let Some(list) = &modifiers {
                for &m in &list.nodes {
                    self.node_mut(m).flags |= NodeFlags::Ambient;
                }
            }
            let save_context_flags = self.context_flags;
            self.set_context_flags(NodeFlags::Ambient, true);
            let result = self.parse_declaration_worker(pos, modifiers);
            self.context_flags = save_context_flags;
            result
        } else {
            self.parse_declaration_worker(pos, modifiers)
        }
    }

    pub(crate) fn parse_declaration_worker(&mut self, pos: usize, modifiers: Option<NodeList>) -> NodeId {
        match self.token {
            Kind::VarKeyword | Kind::LetKeyword | Kind::ConstKeyword | Kind::UsingKeyword => {
                return self.parse_variable_statement(pos, modifiers);
            }
            Kind::AwaitKeyword => {
                if self.is_await_using_declaration() {
                    return self.parse_variable_statement(pos, modifiers);
                }
            }
            Kind::FunctionKeyword => return self.parse_function_declaration(pos, modifiers),
            Kind::ClassKeyword => return self.parse_class_declaration(pos, modifiers),
            Kind::InterfaceKeyword => return self.parse_interface_declaration(pos, modifiers),
            Kind::TypeKeyword => return self.parse_type_alias_declaration(pos, modifiers),
            Kind::EnumKeyword => return self.parse_enum_declaration(pos, modifiers),
            Kind::GlobalKeyword | Kind::ModuleKeyword | Kind::NamespaceKeyword => {
                return self.parse_module_declaration(pos, modifiers);
            }
            Kind::ImportKeyword => return self.parse_import_declaration_or_import_equals_declaration(pos, modifiers),
            Kind::ExportKeyword => {
                self.next_token();
                return match self.token {
                    Kind::DefaultKeyword | Kind::EqualsToken => self.parse_export_assignment(pos, modifiers),
                    Kind::AsKeyword => self.parse_namespace_export_declaration(pos, modifiers),
                    _ => self.parse_export_declaration(pos, modifiers),
                };
            }
            _ => {}
        }
        if modifiers.is_some() {
            // We reached this point because we encountered decorators and/or modifiers and assumed a declaration
            // would follow. For recovery and error reporting purposes, return an incomplete declaration.
            let node_pos = self.node_pos();
            self.parse_error_at(node_pos, node_pos, diagnostics::Declaration_expected, &[]);
            return self.finish_node(Node::new(Kind::MissingDeclaration).modifiers(modifiers), pos);
        }
        unreachable!("Unhandled case in parseDeclarationWorker")
    }

    /// `core.Some(modifiers.Nodes, ...)` / `ModifierFlags & ...` for a single modifier kind.
    pub(crate) fn modifier_list_has(&self, modifiers: &Option<NodeList>, kind: Kind) -> bool {
        modifiers
            .as_ref()
            .is_some_and(|list| list.nodes.iter().any(|&m| self.kind(m) == kind))
    }

    pub(crate) fn is_let_declaration(&mut self) -> bool {
        // In ES6 'let' always starts a lexical declaration if followed by an identifier or {
        // or [.
        self.look_ahead(Self::next_token_is_binding_identifier_or_start_of_destructuring)
    }

    pub(crate) fn next_token_is_binding_identifier_or_start_of_destructuring(&mut self) -> bool {
        self.next_token();
        self.is_binding_identifier() || self.token == Kind::OpenBraceToken || self.token == Kind::OpenBracketToken
    }

    pub(crate) fn parse_block(&mut self, ignore_missing_open_brace: bool, diagnostic_message: Option<&'static Message>) -> NodeId {
        let pos = self.node_pos();
        let open_brace_position = self.scanner.token_start();
        let open_brace_parsed = self.parse_expected_with_diagnostic(Kind::OpenBraceToken, diagnostic_message, true);
        if open_brace_parsed || ignore_missing_open_brace {
            let statements = self.parse_list(PC::BlockStatements, Self::parse_statement);
            self.parse_expected_matching_brackets(
                Kind::OpenBraceToken,
                Kind::CloseBraceToken,
                open_brace_parsed,
                open_brace_position,
            );
            let result = self.finish_node(Node::new(Kind::Block).list(statements), pos);
            if self.token == Kind::EqualsToken {
                self.parse_error_at_current_token(diagnostics::Declaration_or_statement_expected_This_follows_a_block_of_statements_so_if_you_intended_to_write_a_destructuring_assignment_you_might_need_to_wrap_the_whole_assignment_in_parentheses, &[]);
                self.next_token();
            }
            return result;
        }
        let statements = self.create_missing_list();
        self.finish_node(Node::new(Kind::Block).list(statements), pos)
    }

    pub(crate) fn parse_empty_statement(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::SemicolonToken);
        self.finish_node(Node::new(Kind::EmptyStatement), pos)
    }

    pub(crate) fn parse_if_statement(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::IfKeyword);
        let open_paren_position = self.scanner.token_start();
        let open_paren_parsed = self.parse_expected(Kind::OpenParenToken);
        let expression = self.parse_expression_allow_in();
        self.parse_expected_matching_brackets(
            Kind::OpenParenToken,
            Kind::CloseParenToken,
            open_paren_parsed,
            open_paren_position,
        );
        let then_statement = self.parse_statement();
        let mut else_statement = None;
        if self.parse_optional(Kind::ElseKeyword) {
            else_statement = Some(self.parse_statement());
        }
        self.finish_node(
            Node::new(Kind::IfStatement).expression(expression).child(then_statement).child(else_statement),
            pos,
        )
    }

    pub(crate) fn parse_do_statement(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::DoKeyword);
        let statement = self.parse_statement();
        self.parse_expected(Kind::WhileKeyword);
        let open_paren_position = self.scanner.token_start();
        let open_paren_parsed = self.parse_expected(Kind::OpenParenToken);
        let expression = self.parse_expression_allow_in();
        self.parse_expected_matching_brackets(
            Kind::OpenParenToken,
            Kind::CloseParenToken,
            open_paren_parsed,
            open_paren_position,
        );
        // "do{;}while(false)false" is prohibited in the spec but allowed in consensus reality:
        // do;while(0)x will have a semicolon inserted before x.
        self.parse_optional(Kind::SemicolonToken);
        self.finish_node(Node::new(Kind::DoStatement).child(statement).expression(expression), pos)
    }

    pub(crate) fn parse_while_statement(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::WhileKeyword);
        let open_paren_position = self.scanner.token_start();
        let open_paren_parsed = self.parse_expected(Kind::OpenParenToken);
        let expression = self.parse_expression_allow_in();
        self.parse_expected_matching_brackets(
            Kind::OpenParenToken,
            Kind::CloseParenToken,
            open_paren_parsed,
            open_paren_position,
        );
        let statement = self.parse_statement();
        self.finish_node(Node::new(Kind::WhileStatement).expression(expression).child(statement), pos)
    }

    pub(crate) fn parse_for_or_for_in_or_for_of_statement(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::ForKeyword);
        let await_token = self.parse_optional_token(Kind::AwaitKeyword);
        self.parse_expected(Kind::OpenParenToken);
        let mut initializer = None;
        if self.token != Kind::SemicolonToken {
            if self.token == Kind::VarKeyword
                || self.token == Kind::LetKeyword
                || self.token == Kind::ConstKeyword
                || self.token == Kind::UsingKeyword
                    && self.look_ahead(Self::next_token_is_binding_identifier_or_start_of_destructuring_on_same_line_disallow_of)
                // this one is meant to allow of
                || self.token == Kind::AwaitKeyword
                    && self.look_ahead(Self::next_is_using_keyword_then_binding_identifier_or_start_of_object_destructuring_on_same_line)
            {
                initializer = Some(self.parse_variable_declaration_list(true));
            } else {
                initializer = Some(self.do_in_context(NodeFlags::DisallowInContext, true, Self::parse_expression));
            }
        }
        let result;
        if await_token.is_some() && self.parse_expected(Kind::OfKeyword)
            || await_token.is_none() && self.parse_optional(Kind::OfKeyword)
        {
            let expression =
                self.do_in_context(NodeFlags::DisallowInContext, false, Self::parse_assignment_expression_or_higher);
            self.parse_expected(Kind::CloseParenToken);
            let statement = self.parse_statement();
            result = Node::new(Kind::ForOfStatement)
                .child(await_token)
                .initializer(initializer)
                .expression(expression)
                .child(statement);
        } else if self.parse_optional(Kind::InKeyword) {
            let expression = self.parse_expression_allow_in();
            self.parse_expected(Kind::CloseParenToken);
            let statement = self.parse_statement();
            result = Node::new(Kind::ForInStatement)
                .child(None::<NodeId>)
                .initializer(initializer)
                .expression(expression)
                .child(statement);
        } else {
            self.parse_expected(Kind::SemicolonToken);
            let mut condition = None;
            if self.token != Kind::SemicolonToken && self.token != Kind::CloseParenToken {
                condition = Some(self.parse_expression_allow_in());
            }
            self.parse_expected(Kind::SemicolonToken);
            let mut incrementor = None;
            if self.token != Kind::CloseParenToken {
                incrementor = Some(self.parse_expression_allow_in());
            }
            self.parse_expected(Kind::CloseParenToken);
            let statement = self.parse_statement();
            result = Node::new(Kind::ForStatement)
                .initializer(initializer)
                .child(condition)
                .child(incrementor)
                .child(statement);
        }
        self.finish_node(result, pos)
    }

    pub(crate) fn parse_break_statement(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::BreakKeyword);
        let label = self.parse_identifier_unless_at_semicolon();
        self.parse_semicolon();
        self.finish_node(Node::new(Kind::BreakStatement).child(label), pos)
    }

    pub(crate) fn parse_continue_statement(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::ContinueKeyword);
        let label = self.parse_identifier_unless_at_semicolon();
        self.parse_semicolon();
        self.finish_node(Node::new(Kind::ContinueStatement).child(label), pos)
    }

    pub(crate) fn parse_identifier_unless_at_semicolon(&mut self) -> Option<NodeId> {
        if !self.can_parse_semicolon() {
            return Some(self.parse_identifier());
        }
        None
    }

    pub(crate) fn parse_return_statement(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::ReturnKeyword);
        let mut expression = None;
        if !self.can_parse_semicolon() {
            expression = Some(self.parse_expression_allow_in());
        }
        self.parse_semicolon();
        self.finish_node(Node::new(Kind::ReturnStatement).expression(expression), pos)
    }

    pub(crate) fn parse_with_statement(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::WithKeyword);
        let open_paren_position = self.scanner.token_start();
        let open_paren_parsed = self.parse_expected(Kind::OpenParenToken);
        let expression = self.parse_expression_allow_in();
        self.parse_expected_matching_brackets(
            Kind::OpenParenToken,
            Kind::CloseParenToken,
            open_paren_parsed,
            open_paren_position,
        );
        let statement = self.do_in_context(NodeFlags::InWithStatement, true, Self::parse_statement);
        self.finish_node(Node::new(Kind::WithStatement).expression(expression).child(statement), pos)
    }

    pub(crate) fn parse_case_clause(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::CaseKeyword);
        let expression = self.parse_expression_allow_in();
        self.parse_expected(Kind::ColonToken);
        let statements = self.parse_list(PC::SwitchClauseStatements, Self::parse_statement);
        self.finish_node(Node::new(Kind::CaseClause).expression(expression).list(statements), pos)
    }

    pub(crate) fn parse_default_clause(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::DefaultKeyword);
        self.parse_expected(Kind::ColonToken);
        let statements = self.parse_list(PC::SwitchClauseStatements, Self::parse_statement);
        self.finish_node(Node::new(Kind::DefaultClause).list(statements), pos)
    }

    pub(crate) fn parse_case_or_default_clause(&mut self) -> NodeId {
        if self.token == Kind::CaseKeyword {
            return self.parse_case_clause();
        }
        self.parse_default_clause()
    }

    pub(crate) fn parse_case_block(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::OpenBraceToken);
        let clauses = self.parse_list(PC::SwitchClauses, Self::parse_case_or_default_clause);
        self.parse_expected(Kind::CloseBraceToken);
        self.finish_node(Node::new(Kind::CaseBlock).list(clauses), pos)
    }

    pub(crate) fn parse_switch_statement(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::SwitchKeyword);
        self.parse_expected(Kind::OpenParenToken);
        let expression = self.parse_expression_allow_in();
        self.parse_expected(Kind::CloseParenToken);
        let case_block = self.parse_case_block();
        self.finish_node(Node::new(Kind::SwitchStatement).expression(expression).child(case_block), pos)
    }

    pub(crate) fn parse_throw_statement(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::ThrowKeyword);
        // Because of automatic semicolon insertion, a throw that could be terminated with a
        // semicolon gets a missing identifier; the error is reported by the grammar walker.
        let expression = if !self.has_preceding_line_break() {
            self.parse_expression_allow_in()
        } else {
            self.create_missing_identifier()
        };
        if !self.try_parse_semicolon() {
            self.parse_error_for_missing_semicolon_after(expression);
        }
        self.finish_node(Node::new(Kind::ThrowStatement).expression(expression), pos)
    }

    pub(crate) fn parse_try_statement(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::TryKeyword);
        let try_block = self.parse_block(false, None);
        let mut catch_clause = None;
        if self.token == Kind::CatchKeyword {
            catch_clause = Some(self.parse_catch_clause());
        }
        // If we don't have a catch clause, then we must have a finally clause. Try to parse
        // one out no matter what.
        let mut finally_block = None;
        if catch_clause.is_none() || self.token == Kind::FinallyKeyword {
            self.parse_expected_with_diagnostic(Kind::FinallyKeyword, Some(diagnostics::X_catch_or_finally_expected), true);
            finally_block = Some(self.parse_block(false, None));
        }
        self.finish_node(
            Node::new(Kind::TryStatement).child(try_block).child(catch_clause).child(finally_block),
            pos,
        )
    }

    pub(crate) fn parse_catch_clause(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::CatchKeyword);
        let mut variable_declaration = None;
        if self.parse_optional(Kind::OpenParenToken) {
            variable_declaration = Some(self.parse_variable_declaration());
            self.parse_expected(Kind::CloseParenToken);
        }
        let block = self.parse_block(false, None);
        self.finish_node(Node::new(Kind::CatchClause).child(variable_declaration).child(block), pos)
    }

    pub(crate) fn parse_debugger_statement(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::DebuggerKeyword);
        self.parse_semicolon();
        self.finish_node(Node::new(Kind::DebuggerStatement), pos)
    }

    pub(crate) fn parse_expression_or_labeled_statement(&mut self) -> NodeId {
        // Avoiding having to do the lookahead for a labeled statement by just trying to parse
        // out an expression, seeing if it is identifier and then seeing if it is followed by
        // a colon.
        let pos = self.node_pos();
        let expression = self.parse_expression();
        if self.kind(expression) == Kind::Identifier && self.parse_optional(Kind::ColonToken) {
            let statement = self.parse_statement();
            return self.finish_node(Node::new(Kind::LabeledStatement).child(expression).child(statement), pos);
        }
        if !self.try_parse_semicolon() {
            self.parse_error_for_missing_semicolon_after(expression);
        }
        self.finish_node(Node::new(Kind::ExpressionStatement).expression(expression), pos)
    }

    pub(crate) fn parse_variable_statement(&mut self, pos: usize, modifiers: Option<NodeList>) -> NodeId {
        let declaration_list = self.parse_variable_declaration_list(false);
        self.parse_semicolon();
        let result = self.finish_node(
            Node::new(Kind::VariableStatement).modifiers(modifiers).child(declaration_list),
            pos,
        );
        self.check_js_syntax(result)
    }

    pub(crate) fn parse_variable_declaration_list(&mut self, in_for_statement_initializer: bool) -> NodeId {
        let pos = self.node_pos();
        let mut flags = NodeFlags::None;
        match self.token {
            Kind::VarKeyword => flags = NodeFlags::None,
            Kind::LetKeyword => flags = NodeFlags::Let,
            Kind::ConstKeyword => flags = NodeFlags::Const,
            Kind::UsingKeyword => flags = NodeFlags::Using,
            Kind::AwaitKeyword => {
                if self.is_await_using_declaration() {
                    flags = NodeFlags::AwaitUsing;
                    self.next_token();
                }
            }
            _ => unreachable!("Unhandled case in parseVariableDeclarationList"),
        }
        self.next_token();
        // The user may have written `for (let of X) { }`: parse an empty declaration list and
        // then parse 'of' as a keyword ('of' is a valid identifier, so this needs a lookahead).
        let declarations = if self.token == Kind::OfKeyword && self.look_ahead(Self::next_is_identifier_and_close_paren) {
            self.create_missing_list()
        } else {
            let save_context_flags = self.context_flags;
            self.set_context_flags(NodeFlags::DisallowInContext, in_for_statement_initializer);
            let allow_exclamation = !in_for_statement_initializer;
            let declarations = self.parse_delimited_list(PC::VariableDeclarations, move |parser: &mut Self| {
                parser.parse_variable_declaration_worker(allow_exclamation)
            });
            self.context_flags = save_context_flags;
            declarations
        };
        self.finish_node(Node::new(Kind::VariableDeclarationList).list(declarations).flags(flags), pos)
    }

    pub(crate) fn next_is_identifier_and_close_paren(&mut self) -> bool {
        self.next_token_is_identifier() && self.next_token() == Kind::CloseParenToken
    }

    pub(crate) fn next_token_is_identifier(&mut self) -> bool {
        self.next_token();
        self.is_identifier()
    }

    pub(crate) fn parse_variable_declaration(&mut self) -> NodeId {
        self.parse_variable_declaration_worker(false)
    }


    pub(crate) fn parse_variable_declaration_worker(&mut self, allow_exclamation: bool) -> NodeId {
        let pos = self.node_pos();
        let name = self.parse_identifier_or_pattern_with_diagnostic(Some(
            diagnostics::Private_identifiers_are_not_allowed_in_variable_declarations,
        ));
        let mut exclamation_token = None;
        if allow_exclamation
            && self.kind(name) == Kind::Identifier
            && self.token == Kind::ExclamationToken
            && !self.has_preceding_line_break()
        {
            exclamation_token = Some(self.parse_token_node());
        }
        let type_node = self.parse_type_annotation();
        let mut initializer = None;
        if self.token != Kind::InKeyword && self.token != Kind::OfKeyword {
            initializer = self.parse_initializer();
        }
        let result = self.finish_node(
            Node::new(Kind::VariableDeclaration)
                .name(name)
                .child(exclamation_token)
                .ty(type_node)
                .initializer(initializer),
            pos,
        );
        self.check_js_syntax(result)
    }

    pub(crate) fn parse_identifier_or_pattern(&mut self) -> NodeId {
        self.parse_identifier_or_pattern_with_diagnostic(None)
    }

    pub(crate) fn parse_identifier_or_pattern_with_diagnostic(
        &mut self,
        private_identifier_diagnostic_message: Option<&'static Message>,
    ) -> NodeId {
        if self.token == Kind::OpenBracketToken {
            return self.parse_array_binding_pattern();
        }
        if self.token == Kind::OpenBraceToken {
            return self.parse_object_binding_pattern();
        }
        self.parse_binding_identifier_with_diagnostic(private_identifier_diagnostic_message)
    }

    pub(crate) fn parse_array_binding_pattern(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::OpenBracketToken);
        let save_context_flags = self.context_flags;
        self.set_context_flags(NodeFlags::DisallowInContext, false);
        let elements = self.parse_delimited_list(PC::ArrayBindingElements, Self::parse_array_binding_element);
        self.context_flags = save_context_flags;
        self.parse_expected(Kind::CloseBracketToken);
        self.finish_node(Node::new(Kind::ArrayBindingPattern).list(elements), pos)
    }

    pub(crate) fn parse_array_binding_element(&mut self) -> NodeId {
        let pos = self.node_pos();
        let mut dot_dot_dot_token = None;
        let mut name = None;
        let mut initializer = None;
        if self.token != Kind::CommaToken {
            // These are all nil for a missing element
            dot_dot_dot_token = self.parse_optional_token(Kind::DotDotDotToken);
            name = Some(self.parse_identifier_or_pattern());
            initializer = self.parse_initializer();
        }
        self.finish_node(
            Node::new(Kind::BindingElement)
                .child(dot_dot_dot_token)
                .child(None::<NodeId>)
                .name(name)
                .initializer(initializer),
            pos,
        )
    }

    pub(crate) fn parse_object_binding_pattern(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_expected(Kind::OpenBraceToken);
        let save_context_flags = self.context_flags;
        self.set_context_flags(NodeFlags::DisallowInContext, false);
        let elements = self.parse_delimited_list(PC::ObjectBindingElements, Self::parse_object_binding_element);
        self.context_flags = save_context_flags;
        self.parse_expected(Kind::CloseBraceToken);
        self.finish_node(Node::new(Kind::ObjectBindingPattern).list(elements), pos)
    }

    pub(crate) fn parse_object_binding_element(&mut self) -> NodeId {
        let pos = self.node_pos();
        let dot_dot_dot_token = self.parse_optional_token(Kind::DotDotDotToken);
        let token_is_identifier = self.is_binding_identifier();
        let mut property_name = Some(self.parse_property_name());
        let name;
        if token_is_identifier && self.token != Kind::ColonToken {
            name = property_name;
            property_name = None;
        } else {
            self.parse_expected(Kind::ColonToken);
            name = Some(self.parse_identifier_or_pattern());
        }
        let initializer = self.parse_initializer();
        self.finish_node(
            Node::new(Kind::BindingElement)
                .child(dot_dot_dot_token)
                .child(property_name)
                .name(name)
                .initializer(initializer),
            pos,
        )
    }

    pub(crate) fn parse_initializer(&mut self) -> Option<NodeId> {
        if self.parse_optional(Kind::EqualsToken) {
            return Some(self.parse_assignment_expression_or_higher());
        }
        None
    }

    pub(crate) fn parse_type_annotation(&mut self) -> Option<NodeId> {
        if self.parse_optional(Kind::ColonToken) {
            return Some(self.parse_type());
        }
        None
    }

    pub(crate) fn parse_function_declaration(&mut self, pos: usize, modifiers: Option<NodeList>) -> NodeId {
        self.parse_expected(Kind::FunctionKeyword);
        let asterisk_token = self.parse_optional_token(Kind::AsteriskToken);
        // We don't parse the name here in await context, instead we will report a grammar error in the checker.
        let mut name = None;
        if modifiers.is_none() || !self.modifier_list_has(&modifiers, Kind::DefaultKeyword) || self.is_binding_identifier() {
            name = Some(self.parse_binding_identifier());
        }
        let signature_flags = (if asterisk_token.is_some() { ParseFlags::Yield } else { ParseFlags::None })
            | (if self.modifier_list_has(&modifiers, Kind::AsyncKeyword) { ParseFlags::Await } else { ParseFlags::None });
        let type_parameters = self.parse_type_parameters();
        let save_context_flags = self.context_flags;
        if self.modifier_list_has(&modifiers, Kind::ExportKeyword) {
            self.set_context_flags(NodeFlags::AwaitContext, true);
        }
        let parameters = self.parse_parameters(signature_flags);
        let return_type = self.parse_return_type(Kind::ColonToken, false);
        let body = self.parse_function_block_or_semicolon(signature_flags, Some(diagnostics::X_or_expected));
        self.context_flags = save_context_flags;
        let result = self.finish_node(
            Node::new(Kind::FunctionDeclaration)
                .modifiers(modifiers)
                .child(asterisk_token)
                .name(name)
                .type_parameters(type_parameters)
                .parameters(parameters)
                .ty(return_type)
                .body(body),
            pos,
        );
        self.check_js_syntax(result)
    }

    pub(crate) fn parse_class_declaration(&mut self, pos: usize, modifiers: Option<NodeList>) -> NodeId {
        self.parse_class_declaration_or_expression(pos, modifiers, Kind::ClassDeclaration)
    }

    pub(crate) fn parse_class_expression(&mut self) -> NodeId {
        let pos = self.node_pos();
        self.parse_class_declaration_or_expression(pos, None, Kind::ClassExpression)
    }

    pub(crate) fn parse_class_declaration_or_expression(
        &mut self,
        pos: usize,
        modifiers: Option<NodeList>,
        kind: Kind,
    ) -> NodeId {
        let save_context_flags = self.context_flags;
        let save_has_await_identifier = self.statement_has_await_identifier;
        self.parse_expected(Kind::ClassKeyword);
        // We don't parse the name here in await context, instead we will report a grammar error in the checker.
        let name = self.parse_name_of_class_declaration_or_expression();
        let type_parameters = self.parse_type_parameters();
        if self.modifier_list_has(&modifiers, Kind::ExportKeyword) {
            self.set_context_flags(NodeFlags::AwaitContext, true);
        }
        let heritage_clauses = self.parse_heritage_clauses();
        let members = if self.parse_expected(Kind::OpenBraceToken) {
            let members = self.parse_list(PC::ClassMembers, Self::parse_class_element);
            self.parse_expected(Kind::CloseBraceToken);
            members
        } else {
            self.create_missing_list()
        };
        self.context_flags = save_context_flags;
        if self.modifier_list_has(&modifiers, Kind::DeclareKeyword) {
            self.statement_has_await_identifier = save_has_await_identifier;
        }
        let result = self.finish_node(
            Node::new(kind)
                .modifiers(modifiers)
                .name(name)
                .type_parameters(type_parameters)
                .list(heritage_clauses.clone())
                .list(members),
            pos,
        );
        if self.node(result).flags.has(NodeFlags::JavaScriptFile) {
            self.check_js_syntax(result);
            if let Some(heritage_clauses) = heritage_clauses {
                for clause in heritage_clauses.nodes {
                    if self.node(clause).op == Kind::ExtendsKeyword {
                        // slot: HeritageClause's types are its first list.
                        let types = self.node(clause).lists.first().cloned().flatten();
                        if let Some(types) = types {
                            for expr in types.nodes {
                                self.check_js_syntax(expr);
                            }
                        }
                    }
                }
            }
        }
        result
    }

    pub(crate) fn parse_name_of_class_declaration_or_expression(&mut self) -> Option<NodeId> {
        // 'class implements' might mean either a class expression with omitted name, where
        // 'implements' starts a heritage clause, or a class named 'implements'.
        if self.is_binding_identifier() && !self.is_implements_clause() {
            let save_has_await_identifier = self.statement_has_await_identifier;
            let is_binding_identifier = self.is_binding_identifier();
            let id = self.create_identifier(is_binding_identifier);
            self.statement_has_await_identifier = save_has_await_identifier;
            return Some(id);
        }
        None
    }

    pub(crate) fn is_implements_clause(&mut self) -> bool {
        self.token == Kind::ImplementsKeyword && self.look_ahead(Self::next_token_is_identifier_or_keyword)
    }

    pub(crate) fn parse_heritage_clauses(&mut self) -> Option<NodeList> {
        if self.is_heritage_clause() {
            return Some(self.parse_list(PC::HeritageClauses, Self::parse_heritage_clause));
        }
        None
    }

    pub(crate) fn parse_heritage_clause(&mut self) -> NodeId {
        let pos = self.node_pos();
        let kind = self.token;
        self.next_token();
        let types = self.parse_delimited_list(PC::HeritageClauseElement, Self::parse_expression_with_type_arguments);
        let result = self.finish_node(Node::new(Kind::HeritageClause).op(kind).list(types), pos);
        self.check_js_syntax(result)
    }

    pub(crate) fn parse_expression_with_type_arguments(&mut self) -> NodeId {
        let pos = self.node_pos();
        let expression = self.parse_left_hand_side_expression_or_higher();
        if self.kind(expression) == Kind::ExpressionWithTypeArguments {
            return expression;
        }
        let type_arguments = self.parse_type_arguments();
        self.finish_node(
            Node::new(Kind::ExpressionWithTypeArguments).expression(expression).type_arguments(type_arguments),
            pos,
        )
    }

    pub(crate) fn parse_class_element(&mut self) -> NodeId {
        let pos = self.node_pos();
        if self.token == Kind::SemicolonToken {
            self.next_token();
            return self.finish_node(Node::new(Kind::SemicolonClassElement), pos);
        }
        let modifiers = self.parse_modifiers_ex(true, true, true);
        if self.token == Kind::StaticKeyword && self.look_ahead(Self::next_token_is_open_brace) {
            return self.parse_class_static_block_declaration(pos, modifiers);
        }
        if self.parse_contextual_modifier(Kind::GetKeyword) {
            return self.parse_accessor_declaration(pos, modifiers, Kind::GetAccessor, ParseFlags::None);
        }
        if self.parse_contextual_modifier(Kind::SetKeyword) {
            return self.parse_accessor_declaration(pos, modifiers, Kind::SetAccessor, ParseFlags::None);
        }
        if self.token == Kind::ConstructorKeyword || self.token == Kind::StringLiteral {
            let constructor_declaration = self.try_parse_constructor_declaration(pos, modifiers.clone());
            if let Some(constructor_declaration) = constructor_declaration {
                return constructor_declaration;
            }
        }
        if self.is_index_signature() {
            let declaration = self.parse_index_signature_declaration(pos, modifiers);
            return self.check_js_syntax(declaration);
        }
        // It is very important that we check this *after* checking indexers because
        // the [ token can start an index signature or a computed property name
        if token_is_identifier_or_keyword(self.token)
            || self.token == Kind::StringLiteral
            || self.token == Kind::NumericLiteral
            || self.token == Kind::BigIntLiteral
            || self.token == Kind::AsteriskToken
            || self.token == Kind::OpenBracketToken
        {
            let is_ambient = self.modifier_list_has(&modifiers, Kind::DeclareKeyword);
            if is_ambient {
                if let Some(list) = &modifiers {
                    for &m in &list.nodes {
                        self.node_mut(m).flags |= NodeFlags::Ambient;
                    }
                }
                let save_context_flags = self.context_flags;
                self.set_context_flags(NodeFlags::Ambient, true);
                let result = self.parse_property_or_method_declaration(pos, modifiers);
                self.context_flags = save_context_flags;
                return result;
            } else {
                return self.parse_property_or_method_declaration(pos, modifiers);
            }
        }
        if modifiers.is_some() {
            // treat this as a property declaration with a missing name.
            let node_pos = self.node_pos();
            self.parse_error_at(node_pos, node_pos, diagnostics::Declaration_expected, &[]);
            let name = self.create_missing_identifier();
            return self.parse_property_declaration(pos, modifiers, name, None);
        }
        // 'isClassMemberStart' should have hinted not to attempt parsing.
        unreachable!("Should not have attempted to parse class member declaration.")
    }

    pub(crate) fn parse_class_static_block_declaration(&mut self, pos: usize, modifiers: Option<NodeList>) -> NodeId {
        self.parse_expected_token(Kind::StaticKeyword);
        let body = self.parse_class_static_block_body();
        self.finish_node(Node::new(Kind::ClassStaticBlockDeclaration).modifiers(modifiers).body(body), pos)
    }

    pub(crate) fn parse_class_static_block_body(&mut self) -> NodeId {
        let save_context_flags = self.context_flags;
        self.set_context_flags(NodeFlags::YieldContext, false);
        self.set_context_flags(NodeFlags::AwaitContext, true);
        let body = self.parse_block(false, None);
        self.context_flags = save_context_flags;
        body
    }

    pub(crate) fn try_parse_constructor_declaration(&mut self, pos: usize, modifiers: Option<NodeList>) -> Option<NodeId> {
        let state = self.mark();
        if self.token == Kind::ConstructorKeyword
            || self.token == Kind::StringLiteral
                && self.scanner.token_value() == "constructor"
                && self.look_ahead(Self::next_token_is_open_paren)
        {
            self.next_token();
            let type_parameters = self.parse_type_parameters();
            let parameters = self.parse_parameters(ParseFlags::None);
            let return_type = self.parse_return_type(Kind::ColonToken, false);
            let body = self.parse_function_block_or_semicolon(ParseFlags::None, Some(diagnostics::X_or_expected));
            let result = self.finish_node(
                Node::new(Kind::Constructor)
                    .modifiers(modifiers)
                    .type_parameters(type_parameters)
                    .parameters(parameters)
                    .ty(return_type)
                    .body(body),
                pos,
            );
            return Some(self.check_js_syntax(result));
        }
        self.rewind(state);
        None
    }

    pub(crate) fn next_token_is_open_paren(&mut self) -> bool {
        self.next_token() == Kind::OpenParenToken
    }

    pub(crate) fn parse_property_or_method_declaration(&mut self, pos: usize, modifiers: Option<NodeList>) -> NodeId {
        let asterisk_token = self.parse_optional_token(Kind::AsteriskToken);
        let name = self.parse_property_name();
        // Note: this is not legal as per the grammar. But we allow it in the parser and
        // report an error in the grammar checker.
        let question_token = self.parse_optional_token(Kind::QuestionToken);
        if asterisk_token.is_some() || self.token == Kind::OpenParenToken || self.token == Kind::LessThanToken {
            return self.parse_method_declaration(
                pos,
                modifiers,
                asterisk_token,
                name,
                question_token,
                Some(diagnostics::X_or_expected),
            );
        }
        self.parse_property_declaration(pos, modifiers, name, question_token)
    }

    pub(crate) fn parse_method_declaration(
        &mut self,
        pos: usize,
        modifiers: Option<NodeList>,
        asterisk_token: Option<NodeId>,
        name: NodeId,
        question_token: Option<NodeId>,
        diagnostic_message: Option<&'static Message>,
    ) -> NodeId {
        let signature_flags = (if asterisk_token.is_some() { ParseFlags::Yield } else { ParseFlags::None })
            | (if self.modifier_list_has_async(&modifiers) { ParseFlags::Await } else { ParseFlags::None });
        let type_parameters = self.parse_type_parameters();
        let parameters = self.parse_parameters(signature_flags);
        let type_node = self.parse_return_type(Kind::ColonToken, false);
        let body = self.parse_function_block_or_semicolon(signature_flags, diagnostic_message);
        let result = self.finish_node(
            Node::new(Kind::MethodDeclaration)
                .modifiers(modifiers)
                .child(asterisk_token)
                .name(name)
                .question(question_token)
                .type_parameters(type_parameters)
                .parameters(parameters)
                .ty(type_node)
                .body(body),
            pos,
        );
        self.check_js_syntax(result)
    }

    // port: Go's free `modifierListHasAsync` needs the node arena to read modifier kinds.
    pub(crate) fn modifier_list_has_async(&self, modifiers: &Option<NodeList>) -> bool {
        self.modifier_list_has(modifiers, Kind::AsyncKeyword)
    }

    pub(crate) fn parse_property_declaration(
        &mut self,
        pos: usize,
        modifiers: Option<NodeList>,
        name: NodeId,
        question_token: Option<NodeId>,
    ) -> NodeId {
        let mut postfix_token = question_token;
        if postfix_token.is_none() && !self.has_preceding_line_break() {
            postfix_token = self.parse_optional_token(Kind::ExclamationToken);
        }
        let type_node = self.parse_type_annotation();
        let initializer = self.do_in_context(
            NodeFlags::YieldContext | NodeFlags::AwaitContext | NodeFlags::DisallowInContext,
            false,
            Self::parse_initializer,
        );
        self.parse_semicolon_after_property_name(name, type_node, initializer);
        let result = self.finish_node(
            Node::new(Kind::PropertyDeclaration)
                .modifiers(modifiers)
                .name(name)
                .question(postfix_token)
                .ty(type_node)
                .initializer(initializer),
            pos,
        );
        self.check_js_syntax(result)
    }

    pub(crate) fn parse_semicolon_after_property_name(
        &mut self,
        name: NodeId,
        type_node: Option<NodeId>,
        initializer: Option<NodeId>,
    ) {
        if self.token == Kind::AtToken && !self.has_preceding_line_break() {
            self.parse_error_at_current_token(
                diagnostics::Decorators_must_precede_the_name_and_all_keywords_of_property_declarations,
                &[],
            );
            return;
        }
        if self.token == Kind::OpenParenToken {
            self.parse_error_at_current_token(diagnostics::Cannot_start_a_function_call_in_a_type_annotation, &[]);
            self.next_token();
            return;
        }
        if type_node.is_some() && !self.can_parse_semicolon() {
            if initializer.is_some() {
                self.parse_error_at_current_token(diagnostics::X_0_expected, &[token_to_string(Kind::SemicolonToken)]);
            } else {
                self.parse_error_at_current_token(diagnostics::Expected_for_property_initializer, &[]);
            }
            return;
        }
        if self.try_parse_semicolon() {
            return;
        }
        if initializer.is_some() {
            self.parse_error_at_current_token(diagnostics::X_0_expected, &[token_to_string(Kind::SemicolonToken)]);
            return;
        }
        self.parse_error_for_missing_semicolon_after(name);
    }

    pub(crate) fn parse_error_for_missing_semicolon_after(&mut self, node: NodeId) {
        // Tagged template literals are sometimes used in places where only simple strings are allowed, i.e.:
        //   module `M1` {
        //   ^^^^^^^^^^^ This block is parsed as a template literal like module`M1`.
        if self.kind(node) == Kind::TaggedTemplateExpression {
            // slot: TaggedTemplateExpression's template is its body.
            let template = self.node(node).body.expect("a tagged template has a template");
            let (template_pos, template_end) = (self.node(template).pos, self.node(template).end);
            let (start, end) = self.skip_range_trivia(template_pos, template_end);
            self.parse_error_at_range(start, end, diagnostics::Module_declaration_names_may_only_use_or_quoted_strings, &[]);
            return;
        }
        // Otherwise, if this isn't a well-known keyword-like identifier, give the generic fallback message.
        let mut expression_text = String::new();
        if self.kind(node) == Kind::Identifier {
            expression_text = self.node(node).text.clone();
        }
        if expression_text.is_empty() {
            self.parse_error_at_current_token(diagnostics::X_0_expected, &[token_to_string(Kind::SemicolonToken)]);
            return;
        }
        let pos = scanner::skip_trivia(self.source_text, self.node(node).pos);
        let node_end = self.node(node).end;
        // Some known keywords are likely signs of syntax being used improperly.
        match expression_text.as_str() {
            "const" | "let" | "var" => {
                self.parse_error_at(pos, node_end, diagnostics::Variable_declaration_not_allowed_at_this_location, &[]);
                return;
            }
            "declare" => {
                // If a declared node failed to parse, it would have emitted a diagnostic already.
                return;
            }
            "interface" => {
                self.parse_error_for_invalid_name(
                    diagnostics::Interface_name_cannot_be_0,
                    diagnostics::Interface_must_be_given_a_name,
                    Kind::OpenBraceToken,
                );
                return;
            }
            "is" => {
                let token_start = self.scanner.token_start();
                self.parse_error_at(
                    pos,
                    token_start,
                    diagnostics::A_type_predicate_is_only_allowed_in_return_type_position_for_functions_and_methods,
                    &[],
                );
                return;
            }
            "module" | "namespace" => {
                self.parse_error_for_invalid_name(
                    diagnostics::Namespace_name_cannot_be_0,
                    diagnostics::Namespace_must_be_given_a_name,
                    Kind::OpenBraceToken,
                );
                return;
            }
            "type" => {
                self.parse_error_for_invalid_name(
                    diagnostics::Type_alias_name_cannot_be_0,
                    diagnostics::Type_alias_must_be_given_a_name,
                    Kind::EqualsToken,
                );
                return;
            }
            _ => {}
        }
        // The user alternatively might have misspelled or forgotten to add a space after a common keyword.
        let mut suggestion = get_spelling_suggestion_for_strings(&expression_text, VIABLE_KEYWORD_SUGGESTIONS);
        if suggestion.is_empty() {
            suggestion = get_space_suggestion(&expression_text);
        }
        if !suggestion.is_empty() {
            self.parse_error_at(pos, node_end, diagnostics::Unknown_keyword_or_identifier_Did_you_mean_0, &[suggestion.as_str()]);
            return;
        }
        // Unknown tokens are handled with their own errors in the scanner
        if self.token == Kind::Unknown {
            return;
        }
        // Otherwise, we know this some kind of unknown word, not just a missing expected semicolon.
        self.parse_error_at(pos, node_end, diagnostics::Unexpected_keyword_or_identifier, &[]);
    }

    pub(crate) fn parse_error_for_invalid_name(
        &mut self,
        name_diagnostic: &'static Message,
        blank_diagnostic: &'static Message,
        token_if_blank_name: Kind,
    ) {
        if self.token == token_if_blank_name {
            self.parse_error_at_current_token(blank_diagnostic, &[]);
        } else {
            let value = self.scanner.token_value().to_string();
            self.parse_error_at_current_token(name_diagnostic, &[value.as_str()]);
        }
    }

    pub(crate) fn parse_interface_declaration(&mut self, pos: usize, modifiers: Option<NodeList>) -> NodeId {
        self.parse_expected(Kind::InterfaceKeyword);
        let name = self.parse_identifier();
        let type_parameters = self.parse_type_parameters();
        let heritage_clauses = self.parse_heritage_clauses();
        let members = self.parse_object_type_members();
        let result = self.finish_node(
            Node::new(Kind::InterfaceDeclaration)
                .modifiers(modifiers)
                .name(name)
                .type_parameters(type_parameters)
                .list(heritage_clauses)
                .list(members),
            pos,
        );
        self.check_js_syntax(result)
    }

    pub(crate) fn parse_type_alias_declaration(&mut self, pos: usize, modifiers: Option<NodeList>) -> NodeId {
        self.parse_expected(Kind::TypeKeyword);
        if self.has_preceding_line_break() {
            self.parse_error_at_current_token(diagnostics::Line_break_not_permitted_here, &[]);
        }
        let name = self.parse_identifier();
        let type_parameters = self.parse_type_parameters();
        self.parse_expected(Kind::EqualsToken);
        let type_node = if self.token == Kind::IntrinsicKeyword && self.look_ahead(Self::next_is_not_dot) {
            self.parse_keyword_type_node()
        } else {
            self.parse_type()
        };
        self.parse_semicolon();
        let result = self.finish_node(
            Node::new(Kind::TypeAliasDeclaration)
                .modifiers(modifiers)
                .name(name)
                .type_parameters(type_parameters)
                .ty(type_node),
            pos,
        );
        self.check_js_syntax(result)
    }

    pub(crate) fn next_is_not_dot(&mut self) -> bool {
        self.next_token() != Kind::DotToken
    }

    pub(crate) fn parse_enum_member(&mut self) -> NodeId {
        let pos = self.node_pos();
        let name = self.parse_property_name();
        let initializer = self.do_in_context(NodeFlags::DisallowInContext, false, Self::parse_initializer);
        self.finish_node(Node::new(Kind::EnumMember).name(name).initializer(initializer), pos)
    }

    pub(crate) fn parse_enum_declaration(&mut self, pos: usize, modifiers: Option<NodeList>) -> NodeId {
        let save_has_await_identifier = self.statement_has_await_identifier;
        self.parse_expected(Kind::EnumKeyword);
        let name = self.parse_identifier();
        let members = if self.parse_expected(Kind::OpenBraceToken) {
            let save_context_flags = self.context_flags;
            self.set_context_flags(NodeFlags::YieldContext | NodeFlags::AwaitContext, false);
            let members = self.parse_delimited_list(PC::EnumMembers, Self::parse_enum_member);
            self.context_flags = save_context_flags;
            self.parse_expected(Kind::CloseBraceToken);
            members
        } else {
            self.create_missing_list()
        };
        let result = self.finish_node(
            Node::new(Kind::EnumDeclaration).modifiers(modifiers).name(name).list(members),
            pos,
        );
        self.check_js_syntax(result);
        self.statement_has_await_identifier = save_has_await_identifier;
        result
    }

    pub(crate) fn parse_module_declaration(&mut self, pos: usize, modifiers: Option<NodeList>) -> NodeId {
        let mut keyword = Kind::ModuleKeyword;
        if self.token == Kind::GlobalKeyword {
            // global augmentation
            return self.parse_ambient_external_module_declaration(pos, modifiers);
        } else if self.parse_optional(Kind::NamespaceKeyword) {
            keyword = Kind::NamespaceKeyword;
        } else {
            self.parse_expected(Kind::ModuleKeyword);
            if self.token == Kind::StringLiteral {
                return self.parse_ambient_external_module_declaration(pos, modifiers);
            }
        }
        self.parse_module_or_namespace_declaration(pos, modifiers, false, keyword)
    }

    pub(crate) fn parse_ambient_external_module_declaration(&mut self, pos: usize, modifiers: Option<NodeList>) -> NodeId {
        let name;
        let mut keyword = Kind::ModuleKeyword;
        let save_has_await_identifier = self.statement_has_await_identifier;
        if self.token == Kind::GlobalKeyword {
            // parse 'global' as name of global scope augmentation
            name = self.parse_identifier();
            keyword = Kind::GlobalKeyword;
        } else {
            name = self.parse_literal_expression(true);
        }
        let mut body = None;
        if self.token == Kind::OpenBraceToken {
            body = Some(self.parse_module_block());
        } else {
            self.parse_semicolon();
        }
        let result = self.finish_node(
            Node::new(Kind::ModuleDeclaration).modifiers(modifiers).op(keyword).name(name).body(body),
            pos,
        );
        self.statement_has_await_identifier = save_has_await_identifier;
        result
    }

    pub(crate) fn parse_module_block(&mut self) -> NodeId {
        let pos = self.node_pos();
        let statements = if self.parse_expected(Kind::OpenBraceToken) {
            let statements = self.parse_list(PC::BlockStatements, Self::parse_statement);
            self.parse_expected(Kind::CloseBraceToken);
            statements
        } else {
            self.create_missing_list()
        };
        self.finish_node(Node::new(Kind::ModuleBlock).list(statements), pos)
    }

    pub(crate) fn parse_module_or_namespace_declaration(
        &mut self,
        pos: usize,
        modifiers: Option<NodeList>,
        nested: bool,
        keyword: Kind,
    ) -> NodeId {
        let save_has_await_identifier = self.statement_has_await_identifier;
        let name = if nested { self.parse_identifier_name() } else { self.parse_identifier() };
        let body = if self.parse_optional(Kind::DotToken) {
            // port: Go builds the implicit `export` without finishNode (no context flags, and the
            // pending parse-error bit is left for the next finished node), so it is pushed directly.
            let node_pos = self.node_pos();
            let mut implicit_export = Node::new(Kind::ExportKeyword);
            implicit_export.pos = node_pos;
            implicit_export.end = node_pos;
            implicit_export.flags = NodeFlags::Reparsed;
            let implicit_export_id = self.nodes.len() as NodeId;
            self.nodes.push(implicit_export);
            let implicit_modifiers = Some(NodeList::new(node_pos, node_pos, vec![implicit_export_id]));
            let body_pos = self.node_pos();
            self.parse_module_or_namespace_declaration(body_pos, implicit_modifiers, true, keyword)
        } else {
            self.parse_module_block()
        };
        let result = self.finish_node(
            Node::new(Kind::ModuleDeclaration).modifiers(modifiers).op(keyword).name(name).body(body),
            pos,
        );
        self.check_js_syntax(result);
        self.statement_has_await_identifier = save_has_await_identifier;
        result
    }

    pub(crate) fn parse_import_declaration_or_import_equals_declaration(
        &mut self,
        pos: usize,
        modifiers: Option<NodeList>,
    ) -> NodeId {
        self.parse_expected(Kind::ImportKeyword);
        let after_import_pos = self.node_pos();
        // We don't parse the identifier here in await context, instead we will report a grammar error in the checker.
        let save_has_await_identifier = self.statement_has_await_identifier;
        let mut identifier = None;
        if self.is_identifier() {
            identifier = Some(self.parse_identifier());
        }
        let mut phase_modifier = Kind::Unknown;
        let identifier_text = identifier.map(|id| self.node(id).text.clone());
        if identifier_text.as_deref() == Some("type")
            && (self.token != Kind::FromKeyword
                || self.is_identifier() && self.look_ahead(Self::next_token_is_from_keyword_or_equals_token))
            && (self.is_identifier() || self.token_after_import_definitely_produces_import_declaration())
        {
            phase_modifier = Kind::TypeKeyword;
            identifier = None;
            if self.is_identifier() {
                identifier = Some(self.parse_identifier());
            }
        } else if identifier_text.as_deref() == Some("defer") {
            let should_parse_as_defer_modifier = if self.token == Kind::FromKeyword {
                !self.look_ahead(Self::next_token_is_token_string_literal)
            } else {
                self.token != Kind::CommaToken && self.token != Kind::EqualsToken
            };
            if should_parse_as_defer_modifier {
                phase_modifier = Kind::DeferKeyword;
                identifier = None;
                if self.is_identifier() {
                    identifier = Some(self.parse_identifier());
                }
            }
        }
        if let Some(identifier) = identifier
            && !self.token_after_imported_identifier_definitely_produces_import_declaration()
            && phase_modifier != Kind::DeferKeyword
        {
            let declaration =
                self.parse_import_equals_declaration(pos, modifiers, identifier, phase_modifier == Kind::TypeKeyword);
            let import_equals = self.check_js_syntax(declaration);
            // Import= declaration is always parsed in an Await context, no need to reparse
            self.statement_has_await_identifier = save_has_await_identifier;
            return import_equals;
        }
        let import_clause = self.try_parse_import_clause(identifier, after_import_pos, phase_modifier);
        // import clause is always parsed in an Await context
        self.statement_has_await_identifier = save_has_await_identifier;
        let module_specifier = self.parse_module_specifier();
        let attributes = self.try_parse_import_attributes();
        self.parse_semicolon();
        let result = self.finish_node(
            Node::new(Kind::ImportDeclaration)
                .modifiers(modifiers)
                .import_clause(import_clause)
                .child(module_specifier)
                .child(attributes),
            pos,
        );
        self.check_js_syntax(result)
    }

    pub(crate) fn next_token_is_from_keyword_or_equals_token(&mut self) -> bool {
        self.next_token();
        self.token == Kind::FromKeyword || self.token == Kind::EqualsToken
    }

    pub(crate) fn token_after_import_definitely_produces_import_declaration(&self) -> bool {
        self.token == Kind::AsteriskToken || self.token == Kind::OpenBraceToken
    }

    pub(crate) fn token_after_imported_identifier_definitely_produces_import_declaration(&self) -> bool {
        // In `import id ___`, the current token decides whether to produce
        // an ImportDeclaration or ImportEqualsDeclaration.
        self.token == Kind::CommaToken || self.token == Kind::FromKeyword
    }

    pub(crate) fn parse_import_equals_declaration(
        &mut self,
        pos: usize,
        modifiers: Option<NodeList>,
        identifier: NodeId,
        is_type_only: bool,
    ) -> NodeId {
        self.parse_expected(Kind::EqualsToken);
        let module_reference = self.parse_module_reference();
        self.parse_semicolon();
        self.finish_node(
            Node::new(Kind::ImportEqualsDeclaration)
                .modifiers(modifiers)
                .type_only(is_type_only)
                .name(identifier)
                .child(module_reference),
            pos,
        )
    }

    pub(crate) fn parse_module_reference(&mut self) -> NodeId {
        if self.token == Kind::RequireKeyword && self.look_ahead(Self::next_token_is_open_paren) {
            return self.parse_external_module_reference();
        }
        self.parse_entity_name(false, None)
    }

    pub(crate) fn parse_external_module_reference(&mut self) -> NodeId {
        let save_has_await_identifier = self.statement_has_await_identifier;
        let pos = self.node_pos();
        self.parse_expected(Kind::RequireKeyword);
        self.parse_expected(Kind::OpenParenToken);
        let expression = self.parse_module_specifier();
        self.parse_expected(Kind::CloseParenToken);
        let result = self.finish_node(Node::new(Kind::ExternalModuleReference).expression(expression), pos);
        self.statement_has_await_identifier = save_has_await_identifier;
        result
    }

    pub(crate) fn parse_module_specifier(&mut self) -> NodeId {
        if self.token == Kind::StringLiteral {
            return self.parse_literal_expression(true);
        }
        // We allow arbitrary expressions here, even though the grammar only allows string
        // literals. We check to ensure that it is only a string literal later in the grammar
        // check pass.
        self.parse_expression()
    }

    // port: the Go `skipJSDocLeadingAsterisks` parameter is only true for JSDoc `@import` tags,
    // which are not ported, so it is dropped here and in `parse_import_clause`.
    pub(crate) fn try_parse_import_clause(
        &mut self,
        identifier: Option<NodeId>,
        pos: usize,
        phase_modifier: Kind,
    ) -> Option<NodeId> {
        // ImportDeclaration:
        //  import ImportClause from ModuleSpecifier ;
        //  import ModuleSpecifier;
        if identifier.is_some() || self.token == Kind::AsteriskToken || self.token == Kind::OpenBraceToken {
            let import_clause = self.parse_import_clause(identifier, pos, phase_modifier);
            self.parse_expected(Kind::FromKeyword);
            return Some(import_clause);
        }
        None
    }

    pub(crate) fn parse_import_clause(&mut self, identifier: Option<NodeId>, pos: usize, phase_modifier: Kind) -> NodeId {
        // If there was no default import or if there is comma token after default import
        // parse namespace or named imports
        let mut named_bindings = None;
        let save_has_await_identifier = self.statement_has_await_identifier;
        if identifier.is_none() || self.parse_optional(Kind::CommaToken) {
            if self.token == Kind::AsteriskToken {
                named_bindings = Some(self.parse_namespace_import());
            } else {
                named_bindings = Some(self.parse_named_imports());
            }
        }
        let result = self.finish_node(
            Node::new(Kind::ImportClause)
                .type_only(phase_modifier == Kind::TypeKeyword)
                .op(phase_modifier)
                .name(identifier)
                .child(named_bindings),
            pos,
        );
        self.statement_has_await_identifier = save_has_await_identifier;
        result
    }

    pub(crate) fn parse_namespace_import(&mut self) -> NodeId {
        // NameSpaceImport:
        //  * as ImportedBinding
        let pos = self.node_pos();
        self.parse_expected(Kind::AsteriskToken);
        self.parse_expected(Kind::AsKeyword);
        let name = self.parse_identifier();
        self.finish_node(Node::new(Kind::NamespaceImport).name(name), pos)
    }

    pub(crate) fn parse_named_imports(&mut self) -> NodeId {
        let pos = self.node_pos();
        let imports = self.parse_bracketed_list(
            PC::ImportOrExportSpecifiers,
            Self::parse_import_specifier,
            Kind::OpenBraceToken,
            Kind::CloseBraceToken,
        );
        self.finish_node(Node::new(Kind::NamedImports).list(imports), pos)
    }

    pub(crate) fn parse_import_specifier(&mut self) -> NodeId {
        let pos = self.node_pos();
        let (is_type_only, property_name, name) = self.parse_import_or_export_specifier(Kind::ImportSpecifier);
        let identifier_name = if self.kind(name) == Kind::Identifier {
            name
        } else {
            let (name_pos, name_end) = (self.node(name).pos, self.node(name).end);
            let (start, end) = self.skip_range_trivia(name_pos, name_end);
            self.parse_error_at_range(start, end, diagnostics::Identifier_expected, &[]);
            let identifier = self.new_identifier(String::new());
            self.finish_node(identifier, name_pos)
        };
        let result = self.finish_node(
            Node::new(Kind::ImportSpecifier)
                .type_only(is_type_only)
                .child(property_name)
                .name(identifier_name),
            pos,
        );
        self.check_js_syntax(result)
    }

    pub(crate) fn parse_import_or_export_specifier(&mut self, kind: Kind) -> (bool, Option<NodeId>, NodeId) {
        // ImportSpecifier:
        //   BindingIdentifier
        //   ModuleExportName as BindingIdentifier
        // ExportSpecifier:
        //   ModuleExportName
        //   ModuleExportName as ModuleExportName
        let mut is_type_only = false;
        let mut property_name = None;
        let mut can_parse_as_keyword = true;
        let disallow_keywords = kind == Kind::ImportSpecifier;
        let (mut name, mut name_ok) = self.parse_module_export_name(disallow_keywords);
        if self.kind(name) == Kind::Identifier && self.node(name).text == "type" {
            // If the first token of an import specifier is 'type', there are a lot of possibilities,
            // especially if we see 'as' afterwards:
            //
            // import { type } from "mod";          - isTypeOnly: false,   name: type
            // import { type as } from "mod";       - isTypeOnly: true,    name: as
            // import { type as as } from "mod";    - isTypeOnly: false,   name: as,    propertyName: type
            // import { type as as as } from "mod"; - isTypeOnly: true,    name: as,    propertyName: as
            if self.token == Kind::AsKeyword {
                // { type as ...? }
                let first_as = self.parse_identifier_name();
                if self.token == Kind::AsKeyword {
                    // { type as as ...? }
                    let second_as = self.parse_identifier_name();
                    if self.can_parse_module_export_name() {
                        // { type as as something }
                        // { type as as "something" }
                        is_type_only = true;
                        property_name = Some(first_as);
                        (name, name_ok) = self.parse_module_export_name(disallow_keywords);
                        can_parse_as_keyword = false;
                    } else {
                        // { type as as }
                        property_name = Some(name);
                        name = second_as;
                        can_parse_as_keyword = false;
                    }
                } else if self.can_parse_module_export_name() {
                    // { type as something }
                    // { type as "something" }
                    property_name = Some(name);
                    can_parse_as_keyword = false;
                    (name, name_ok) = self.parse_module_export_name(disallow_keywords);
                } else {
                    // { type as }
                    is_type_only = true;
                    name = first_as;
                }
            } else if self.can_parse_module_export_name() {
                // { type something ...? }
                // { type "something" ...? }
                is_type_only = true;
                (name, name_ok) = self.parse_module_export_name(disallow_keywords);
            }
        }
        if can_parse_as_keyword && self.token == Kind::AsKeyword {
            property_name = Some(name);
            self.parse_expected(Kind::AsKeyword);
            (name, name_ok) = self.parse_module_export_name(disallow_keywords);
        }
        if !name_ok {
            let (name_pos, name_end) = (self.node(name).pos, self.node(name).end);
            let (start, end) = self.skip_range_trivia(name_pos, name_end);
            self.parse_error_at_range(start, end, diagnostics::Identifier_expected, &[]);
        }
        (is_type_only, property_name, name)
    }

    pub(crate) fn can_parse_module_export_name(&self) -> bool {
        token_is_identifier_or_keyword(self.token) || self.token == Kind::StringLiteral
    }

    pub(crate) fn parse_module_export_name(&mut self, disallow_keywords: bool) -> (NodeId, bool) {
        let mut name_ok = true;
        if self.token == Kind::StringLiteral {
            return (self.parse_literal_expression(false), name_ok);
        }
        if disallow_keywords && ast::is_keyword(self.token) && !self.is_identifier() {
            name_ok = false;
        }
        (self.parse_identifier_name(), name_ok)
    }

    pub(crate) fn try_parse_import_attributes(&mut self) -> Option<NodeId> {
        if self.token == Kind::WithKeyword || (self.token == Kind::AssertKeyword && !self.has_preceding_line_break()) {
            if self.token == Kind::AssertKeyword {
                self.parse_error_at_current_token(
                    diagnostics::Import_assertions_have_been_replaced_by_import_attributes_Use_with_instead_of_assert,
                    &[],
                );
            }
            let token = self.token;
            return Some(self.parse_import_attributes(token, false));
        }
        None
    }

    pub(crate) fn parse_export_assignment(&mut self, pos: usize, modifiers: Option<NodeList>) -> NodeId {
        let save_context_flags = self.context_flags;
        let save_has_await_identifier = self.statement_has_await_identifier;
        self.set_context_flags(NodeFlags::AwaitContext, true);
        let mut is_export_equals = false;
        if self.parse_optional(Kind::EqualsToken) {
            is_export_equals = true;
        } else {
            self.parse_expected(Kind::DefaultKeyword);
        }
        let expression = self.parse_assignment_expression_or_higher();
        self.parse_semicolon();
        self.context_flags = save_context_flags;
        self.statement_has_await_identifier = save_has_await_identifier;
        let result = self.finish_node(
            Node::new(Kind::ExportAssignment)
                .modifiers(modifiers)
                .export_equals(is_export_equals)
                .expression(expression),
            pos,
        );
        self.check_js_syntax(result)
    }

    pub(crate) fn parse_namespace_export_declaration(&mut self, pos: usize, modifiers: Option<NodeList>) -> NodeId {
        self.parse_expected(Kind::AsKeyword);
        self.parse_expected(Kind::NamespaceKeyword);
        let save_has_await_identifier = self.statement_has_await_identifier;
        let name = self.parse_identifier();
        self.statement_has_await_identifier = save_has_await_identifier;
        self.parse_semicolon();
        // NamespaceExportDeclaration nodes cannot have decorators or modifiers, we attach them here so we can report them in the grammar checker
        self.finish_node(Node::new(Kind::NamespaceExportDeclaration).modifiers(modifiers).name(name), pos)
    }

    pub(crate) fn parse_export_declaration(&mut self, pos: usize, modifiers: Option<NodeList>) -> NodeId {
        let save_context_flags = self.context_flags;
        let save_has_await_identifier = self.statement_has_await_identifier;
        self.set_context_flags(NodeFlags::AwaitContext, true);
        let mut export_clause = None;
        let mut module_specifier = None;
        let mut attributes = None;
        let is_type_only = self.parse_optional(Kind::TypeKeyword);
        let namespace_export_pos = self.node_pos();
        if self.parse_optional(Kind::AsteriskToken) {
            if self.parse_optional(Kind::AsKeyword) {
                export_clause = Some(self.parse_namespace_export(namespace_export_pos));
            }
            self.parse_expected(Kind::FromKeyword);
            module_specifier = Some(self.parse_module_specifier());
        } else {
            export_clause = Some(self.parse_named_exports());
            // It is not uncommon to accidentally omit the 'from' keyword. Additionally, in editing scenarios,
            // the 'from' keyword can be parsed as a named export when the export clause is unterminated (i.e. `export { from "moduleName";`)
            // If we don't have a 'from' keyword, see if we have a string literal such that ASI won't take effect.
            if self.token == Kind::FromKeyword || (self.token == Kind::StringLiteral && !self.has_preceding_line_break()) {
                self.parse_expected(Kind::FromKeyword);
                module_specifier = Some(self.parse_module_specifier());
            }
        }
        if module_specifier.is_some()
            && (self.token == Kind::WithKeyword || self.token == Kind::AssertKeyword)
            && !self.has_preceding_line_break()
        {
            if self.token == Kind::AssertKeyword {
                self.parse_error_at_current_token(
                    diagnostics::Import_assertions_have_been_replaced_by_import_attributes_Use_with_instead_of_assert,
                    &[],
                );
            }
            let token = self.token;
            attributes = Some(self.parse_import_attributes(token, false));
        }
        self.parse_semicolon();
        self.context_flags = save_context_flags;
        self.statement_has_await_identifier = save_has_await_identifier;
        let result = self.finish_node(
            Node::new(Kind::ExportDeclaration)
                .modifiers(modifiers)
                .type_only(is_type_only)
                .child(export_clause)
                .child(module_specifier)
                .child(attributes),
            pos,
        );
        self.check_js_syntax(result)
    }

    pub(crate) fn parse_namespace_export(&mut self, pos: usize) -> NodeId {
        let (export_name, _) = self.parse_module_export_name(false);
        self.finish_node(Node::new(Kind::NamespaceExport).name(export_name), pos)
    }

    pub(crate) fn parse_named_exports(&mut self) -> NodeId {
        let pos = self.node_pos();
        let exports = self.parse_bracketed_list(
            PC::ImportOrExportSpecifiers,
            Self::parse_export_specifier,
            Kind::OpenBraceToken,
            Kind::CloseBraceToken,
        );
        self.finish_node(Node::new(Kind::NamedExports).list(exports), pos)
    }

    pub(crate) fn parse_export_specifier(&mut self) -> NodeId {
        let pos = self.node_pos();
        let (is_type_only, property_name, name) = self.parse_import_or_export_specifier(Kind::ExportSpecifier);
        let result = self.finish_node(
            Node::new(Kind::ExportSpecifier)
                .type_only(is_type_only)
                .child(property_name)
                .name(name),
            pos,
        );
        self.check_js_syntax(result)
    }
}

// port: Go builds this list by ranging over the `textToKeyword` map, so its order is random;
// only `get_space_suggestion` depends on the order (when two keywords are prefixes of the
// text), and this port uses alphabetical order.
const VIABLE_KEYWORD_SUGGESTIONS: &[&str] = &[
    "abstract", "accessor", "any", "assert", "asserts", "async", "await", "bigint", "boolean", "break", "case",
    "catch", "class", "const", "constructor", "continue", "debugger", "declare", "default", "defer", "delete",
    "else", "enum", "export", "extends", "false", "finally", "for", "from", "function", "get", "global",
    "immediate", "implements", "import", "infer", "instanceof", "interface", "intrinsic", "keyof", "let",
    "module", "namespace", "never", "new", "null", "number", "object", "out", "override", "package", "private",
    "protected", "public", "readonly", "require", "return", "satisfies", "set", "static", "string", "super",
    "switch", "symbol", "this", "throw", "true", "try", "type", "typeof", "undefined", "unique", "unknown",
    "using", "var", "void", "while", "with", "yield",
];

fn get_space_suggestion(expression_text: &str) -> String {
    for keyword in VIABLE_KEYWORD_SUGGESTIONS {
        if expression_text.len() > keyword.len() + 2 && expression_text.starts_with(*keyword) {
            return format!("{} {}", keyword, &expression_text[keyword.len()..]);
        }
    }
    String::new()
}

/// `core.GetSpellingSuggestionForStrings`.
pub(crate) fn get_spelling_suggestion_for_strings(name: &str, candidates: &[&str]) -> String {
    let rune_name: Vec<char> = name.chars().collect();
    let maximum_length_difference = std::cmp::max(2, (rune_name.len() as f64 * 0.34) as usize);
    // If the best result is worse than this, don't bother.
    let mut best_distance = (rune_name.len() as f64 * 0.4).floor() + 0.9;
    let mut best_candidate: Option<&str> = None;
    for &candidate_name in candidates {
        let max_len = std::cmp::max(candidate_name.len(), rune_name.len());
        let min_len = std::cmp::min(candidate_name.len(), rune_name.len());
        if !candidate_name.is_empty() && max_len - min_len <= maximum_length_difference {
            if candidate_name == name {
                continue;
            }
            // Only consider candidates less than 3 characters long when they differ by case.
            if candidate_name.len() < 3 && !candidate_name.eq_ignore_ascii_case(name) {
                continue;
            }
            let candidate_runes: Vec<char> = candidate_name.chars().collect();
            let Some(distance) = levenshtein_with_max(&rune_name, &candidate_runes, best_distance) else {
                continue;
            };
            if distance < best_distance {
                best_distance = distance;
                best_candidate = Some(candidate_name);
            } else if best_candidate.is_none_or(|best| candidate_name < best) {
                best_candidate = Some(candidate_name);
            }
        }
    }
    best_candidate.unwrap_or("").to_string()
}

fn to_lower(ch: char) -> char {
    ch.to_lowercase().next().unwrap_or(ch)
}

/// `core.levenshteinWithMax`; `None` is Go's `-1`.
fn levenshtein_with_max(s1: &[char], s2: &[char], max_value: f64) -> Option<f64> {
    let buffer_size = s2.len() + 1;
    let mut previous = vec![0.0f64; buffer_size];
    let mut current = vec![0.0f64; buffer_size];
    let big = max_value + 0.01;
    for (i, value) in previous.iter_mut().enumerate() {
        *value = i as f64;
    }
    for i in 1..=s1.len() {
        let c1 = s1[i - 1];
        let min_j = std::cmp::max((i as f64 - max_value).ceil() as i64, 1) as usize;
        let max_j = std::cmp::min((max_value + i as f64).floor() as i64, s2.len() as i64).max(0) as usize;
        let mut col_min = i as f64;
        current[0] = col_min;
        for slot in current.iter_mut().take(min_j.min(buffer_size)).skip(1) {
            *slot = big;
        }
        for j in min_j..=max_j {
            let substitution_distance = if to_lower(s1[i - 1]) == to_lower(s2[j - 1]) {
                previous[j - 1] + 0.1
            } else {
                previous[j - 1] + 2.0
            };
            let dist = if c1 == s2[j - 1] {
                previous[j - 1]
            } else {
                f64::min(previous[j] + 1.0, f64::min(current[j - 1] + 1.0, substitution_distance))
            };
            current[j] = dist;
            col_min = f64::min(col_min, dist);
        }
        for slot in current.iter_mut().skip(max_j + 1) {
            *slot = big;
        }
        if col_min > max_value {
            // Give up -- everything in this column is > max and it can't get better in future columns.
            return None;
        }
        std::mem::swap(&mut previous, &mut current);
    }
    let res = previous[s2.len()];
    if res > max_value {
        return None;
    }
    Some(res)
}
