extern crate swc_common;
extern crate swc_ecma_ast;
extern crate swc_ecma_parser;
extern crate swc_ecma_visit;

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::vec;

use swc_ecma_ast::{
  ArrowExpr, AssignExpr, BlockStmtOrExpr, Callee, ClassDecl, ClassMethod, Constructor, Decl,
  ExportDecl, FnDecl, ObjectLit, PatOrExpr, Prop, PropName, PropOrSpread, Stmt, TryStmt,
  VarDeclarator,
};

use self::swc_common::{comments::Comments, sync::Lrc, Span};
use self::swc_ecma_ast::{
  CallExpr, Expr, Function, ImportDecl, ImportSpecifier, MemberProp, ModuleExportName, ThrowStmt,
  Lit,
};

use self::swc_ecma_visit::Visit;

fn prop_name_to_string(prop_name: &PropName) -> String {
  match prop_name {
    PropName::Ident(ident) => ident.sym.to_string(),
    PropName::Str(str_) => str_.value.to_string(),
    PropName::Num(num) => num.value.to_string(),
    _ => "anonymous".to_string(), // Fallback for unnamed functions
  }
}

#[derive(Clone, Debug)]
pub struct ThrowDetails {
  pub error_type: Option<String>,    // "Error", "TypeError", etc.
  pub error_message: Option<String>, // Literal string if available
  pub is_custom_error: bool,         // true for custom classes
}

impl Default for ThrowDetails {
  fn default() -> Self {
    ThrowDetails {
      error_type: None,
      error_message: None,
      is_custom_error: false,
    }
  }
}

#[derive(Clone, Debug)]
pub struct ThrowsAnnotation {
  pub error_types: Vec<String>,          // ["Error", "TypeError"] 
  pub is_documented: bool,               // Has throws annotation
}

#[derive(Clone)]
struct BlockContext {
  try_count: usize,
  catch_count: usize,
}

pub struct ThrowFinderSettings<'throwfinder_settings> {
  pub include_try_statements: &'throwfinder_settings bool,
  pub ignore_statements: &'throwfinder_settings Vec<String>,
}

impl<'throwfinder_settings> Clone for ThrowFinderSettings<'throwfinder_settings> {
  fn clone(&self) -> ThrowFinderSettings<'throwfinder_settings> {
      ThrowFinderSettings {
          include_try_statements: self.include_try_statements,
          ignore_statements: self.ignore_statements,
      }
  }
}

pub struct ThrowFinder<'throwfinder_settings> {
  comments: Lrc<dyn Comments>,
  pub throw_spans: Vec<Span>,
  pub throw_details: Vec<ThrowDetails>, // NEW: Store error details for each throw
  context_stack: Vec<BlockContext>, // Stack to track try/catch context
  pub throwfinder_settings: &'throwfinder_settings ThrowFinderSettings<'throwfinder_settings>,
}

impl<'throwfinder_settings> ThrowFinder<'throwfinder_settings> {
  fn current_context(&self) -> Option<&BlockContext> {
    self.context_stack.last()
  }

  pub fn new(throwfinder_settings: &'throwfinder_settings ThrowFinderSettings<'throwfinder_settings>, comments: Lrc<dyn Comments>) -> Self {
    Self {
      comments,
      throw_spans: vec![],
      throw_details: vec![], // NEW: Initialize empty details vector
      context_stack: vec![],
      throwfinder_settings,
    }
  }

  fn analyze_throw_expression(&self, expr: &Expr) -> ThrowDetails {
    match expr {
      // new Error("message")
      Expr::New(new_expr) => {
        if let Expr::Ident(ident) = &*new_expr.callee {
          let error_type = ident.sym.to_string();
          let message = new_expr.args.as_ref()
            .and_then(|args| args.first())
            .and_then(|arg| self.extract_string_literal(&arg.expr));

          ThrowDetails {
            error_type: Some(error_type.clone()),
            error_message: message,
            is_custom_error: !is_built_in_error(&error_type),
          }
        } else {
          ThrowDetails::default()
        }
      }
      // throw "string literal"
      Expr::Lit(lit) => {
        if let Lit::Str(str_lit) = lit {
          ThrowDetails {
            error_type: None,
            error_message: Some(str_lit.value.to_string()),
            is_custom_error: false,
          }
        } else {
          ThrowDetails::default()
        }
      }
      // throw variable
      Expr::Ident(ident) => {
        ThrowDetails {
          error_type: Some(format!("variable: {}", ident.sym)),
          error_message: None,
          is_custom_error: false,
        }
      }
      _ => ThrowDetails::default()
    }
  }

  fn extract_string_literal(&self, expr: &Expr) -> Option<String> {
    if let Expr::Lit(Lit::Str(str_lit)) = expr {
      Some(str_lit.value.to_string())
    } else {
      None
    }
  }
}

fn is_built_in_error(name: &str) -> bool {
  matches!(name, "Error" | "TypeError" | "ReferenceError" | "RangeError" |
                 "SyntaxError" | "URIError" | "EvalError" | "AggregateError")
}

impl<'throwfinder_settings> Visit for ThrowFinder<'throwfinder_settings> {
  fn visit_throw_stmt(&mut self, node: &ThrowStmt) {
    let has_it_throws_comment = self
      .comments
      .get_leading(node.span.lo())
      .filter(|comments| {
        comments.iter().any(|c| {
          self
            .throwfinder_settings
            .ignore_statements
            .iter()
            .any(|keyword| c.text.contains(&**keyword))
        })
      })
      .is_some();

    if !has_it_throws_comment {
      // NEW: Extract error details from the throw expression
      let throw_details = self.analyze_throw_expression(&node.arg);

      if *self.throwfinder_settings.include_try_statements {
        self.throw_spans.push(node.span);
        self.throw_details.push(throw_details); // NEW: Store details
      } else {
        let context = self.current_context();
        if context.map_or(true, |ctx| ctx.try_count == ctx.catch_count) {
          // Add throw span if not within an unbalanced try block
          self.throw_spans.push(node.span);
          self.throw_details.push(throw_details); // NEW: Store details
        }
      }
    }
  }

  fn visit_try_stmt(&mut self, node: &TryStmt) {
    // Entering a try block
    let current_context = self.current_context().cloned().unwrap_or(BlockContext {
      try_count: 0,
      catch_count: 0,
    });
    self.context_stack.push(BlockContext {
      try_count: current_context.try_count + 1,
      catch_count: current_context.catch_count,
    });
    swc_ecma_visit::visit_block_stmt_or_expr(self, &BlockStmtOrExpr::BlockStmt(node.block.clone()));

    if let Some(catch_clause) = &node.handler {
      let catch_context = self.context_stack.last_mut().unwrap();
      catch_context.catch_count += 1; // Increment catch count within the same try context
      swc_ecma_visit::visit_catch_clause(self, catch_clause);
    }

    // Leaving try-catch block
    self.context_stack.pop();
  }
}

#[derive(Clone)]
pub struct IdentifierUsage {
  pub usage_span: Span,
  pub identifier_name: String,
  pub usage_context: String,
  pub id: String,
}

impl IdentifierUsage {
  pub fn new(usage_span: Span, identifier_name: String, usage_context: String, id: String) -> Self {
    Self {
      usage_span,
      identifier_name,
      usage_context,
      id,
    }
  }
}

impl Eq for IdentifierUsage {}

impl PartialEq for IdentifierUsage {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl Hash for IdentifierUsage {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.id.hash(state);
    self.usage_span.lo.hash(state);
    self.usage_span.hi.hash(state);
  }
}

#[derive(Clone)]
pub struct ThrowMap {
  pub throw_spans: Vec<Span>,
  pub throw_statement: Span,
  pub function_or_method_name: String,
  pub class_name: Option<String>,
  pub id: String,
  pub throw_details: Vec<ThrowDetails>,             // NEW: Error details for each throw
  pub throws_annotation: Option<ThrowsAnnotation>,  // NEW: Function-level throws annotation
}

impl PartialEq for ThrowMap {
  fn eq(&self, other: &Self) -> bool {
    self.throw_statement == other.throw_statement
  }
}

impl Eq for ThrowMap {}

impl Hash for ThrowMap {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.throw_statement.lo.hash(state);
    self.throw_statement.hi.hash(state);
    self.throw_statement.ctxt.hash(state);
  }
}

pub struct ThrowAnalyzer<'throwfinder_settings> {
  pub comments: Lrc<dyn Comments>,
  pub functions_with_throws: HashSet<ThrowMap>,
  pub json_parse_calls: Vec<String>,
  pub fs_access_calls: Vec<String>,
  pub import_sources: HashSet<String>,
  pub imported_identifiers: Vec<String>,
  pub function_name_stack: Vec<String>,
  pub current_class_name: Option<String>,
  pub current_method_name: Option<String>,
  pub throwfinder_settings: ThrowFinderSettings<'throwfinder_settings>,
}

impl<'throwfinder_settings> ThrowAnalyzer<'throwfinder_settings> {
  fn check_function_for_throws(&mut self, function: &Function) {
    let mut throw_finder = ThrowFinder::new(&self.throwfinder_settings, self.comments.clone());
    throw_finder.visit_function(function);
    if !throw_finder.throw_spans.is_empty() {
      // NEW: Extract throws annotation from function comments
      let throws_annotation = self.extract_throws_annotation(function.span);

      let throw_map = ThrowMap {
        throw_spans: throw_finder.throw_spans,
        throw_statement: function.span,
        function_or_method_name: self
          .function_name_stack
          .last()
          .cloned()
          .unwrap_or_else(|| "<anonymous>".to_string()),
        class_name: None,
        id: format!(
          "{}-{}",
          self
            .current_class_name
            .clone()
            .unwrap_or_else(|| "NOT_SET".to_string()),
          self
            .function_name_stack
            .last()
            .cloned()
            .unwrap_or_else(|| "<anonymous>".to_string())
        ),
        throw_details: throw_finder.throw_details,  // NEW: Pass error details from ThrowFinder
        throws_annotation,                          // NEW: Add throws annotation
      };
      self.functions_with_throws.insert(throw_map);
    }
  }

  fn extract_throws_annotation(&self, function_span: Span) -> Option<ThrowsAnnotation> {
    // Simple approach: Look for leading comments before the function
    // Also search in a range before the function to catch comments attached to parent declarations
    
    // Strategy 1: Direct leading comments at function start
    if let Some(comments) = self.comments.get_leading(function_span.lo()) {
      for comment in comments {
        if let Some(annotation) = self.parse_throws_comment(&comment.text) {
          #[cfg(debug_assertions)]
          eprintln!("   ✅ Found throws annotation in direct leading comment: {:?}", annotation);
          return Some(annotation);
        }
      }
    }
    
    // Strategy 2: Search backwards for comments (to catch variable declaration comments)
    for offset in 1..50 {  // Reduced range to prevent cross-function contamination
      let search_pos = if function_span.lo().0 >= offset {
        function_span.lo() - swc_common::BytePos(offset)
      } else {
        swc_common::BytePos(0)
      };
      
      if let Some(comments) = self.comments.get_leading(search_pos) {
        for comment in comments {
          if let Some(annotation) = self.parse_throws_comment(&comment.text) {
            #[cfg(debug_assertions)]
            eprintln!("   ✅ Found throws annotation in backward search at -{}: {:?}", offset, annotation);
            return Some(annotation);
          }
        }
      }
      
      // Stop searching if we've gone too far back
      if search_pos.0 == 0 {
        break;
      }
    }

    #[cfg(debug_assertions)]
    eprintln!("   ❌ No throws annotation found in leading comments for function span {:?}", function_span);
    
    None
  }

  fn parse_throws_comment(&self, comment_text: &str) -> Option<ThrowsAnnotation> {
    // Only support JSDoc @throws syntax:
    // /** @throws {ErrorType} description */
    // /**
    //  * Description here
    //  * @throws {TypeError} when input is invalid
    //  * @throws {ValidationError} when validation fails
    //  */
    let text = comment_text.trim();
    let mut error_types: std::collections::HashSet<String> = std::collections::HashSet::new(); // Use HashSet to deduplicate

    let lines: Vec<&str> = text.lines()
      .map(|line| line.trim().trim_start_matches('*').trim())
      .collect();

    for line in &lines {
      if line.to_lowercase().contains("@throws") {
        if let Some(throws_pos) = line.to_lowercase().find("@throws") {
          let after_throws = &line[throws_pos + 7..].trim(); // Skip "@throws"

          // Handle @throws {Type} syntax
          if let Some(start_brace) = after_throws.find('{') {
            if let Some(end_brace) = after_throws.find('}') {
              let type_name = &after_throws[start_brace + 1..end_brace].trim();
              if !type_name.is_empty() {
                error_types.insert(type_name.to_string());
                continue;
              }
            }
          }
          // Handle @throws Type (without braces) - extract comma-separated types
          if !after_throws.starts_with('{') {
            let type_section = after_throws
              .split_whitespace()
              .take_while(|word| {
                !["when", "if", "where", "that", "which", "while", "because", "since"].contains(&word.to_lowercase().as_str())
              })
              .collect::<Vec<_>>()
              .join(" ");
            let types: Vec<String> = type_section
              .split(',')
              .map(|s| s.trim().trim_end_matches(',').trim().to_string())
              .filter(|s| !s.is_empty())
              .collect();
            for error_type in types {
              error_types.insert(error_type);
            }
          }
        }
      }
    }
    if !error_types.is_empty() {
      Some(ThrowsAnnotation { 
        error_types: error_types.into_iter().collect(), // Convert HashSet back to Vec
        is_documented: true 
      })
    } else {
      None
    }
  }

  fn check_arrow_function_for_throws(&mut self, arrow_function: &ArrowExpr) {
    let mut throw_finder = ThrowFinder::new(&self.throwfinder_settings, self.comments.clone());
    throw_finder.visit_arrow_expr(arrow_function);
    if !throw_finder.throw_spans.is_empty() {
      // NEW: Extract throws annotation from arrow function comments
      let throws_annotation = self.extract_throws_annotation(arrow_function.span);

      let throw_map = ThrowMap {
        throw_spans: throw_finder.throw_spans,
        throw_statement: arrow_function.span,
        function_or_method_name: self
          .function_name_stack
          .last()
          .cloned()
          .unwrap_or_else(|| "<anonymous>".to_string()),
        class_name: None,
        id: format!(
          "{}-{}",
          self
            .current_class_name
            .clone()
            .unwrap_or_else(|| "NOT_SET".to_string()),
          self
            .function_name_stack
            .last()
            .cloned()
            .unwrap_or_else(|| "<anonymous>".to_string())
        ),
        throw_details: throw_finder.throw_details,  // NEW: Pass error details from ThrowFinder
        throws_annotation,                          // NEW: Add throws annotation
      };
      self.functions_with_throws.insert(throw_map);
    }
  }

  fn check_constructor_for_throws(&mut self, constructor: &Constructor) {
    let mut throw_finder = ThrowFinder::new(&self.throwfinder_settings, self.comments.clone());
    throw_finder.visit_constructor(constructor);
    if !throw_finder.throw_spans.is_empty() {
      // NEW: Extract throws annotation from constructor comments
      let throws_annotation = self.extract_throws_annotation(constructor.span);

      let throw_map = ThrowMap {
        throw_spans: throw_finder.throw_spans,
        throw_statement: constructor.span,
        function_or_method_name: self
          .current_method_name
          .clone()
          .unwrap_or_else(|| "<constructor>".to_string()),
        class_name: self.current_class_name.clone(),
        id: format!(
          "{}-{}",
          self
            .current_class_name
            .clone()
            .unwrap_or_else(|| "NOT_SET".to_string()),
          self
            .current_method_name
            .clone()
            .unwrap_or_else(|| "<constructor>".to_string())
        ),
        throw_details: throw_finder.throw_details,  // NEW: Pass error details from ThrowFinder
        throws_annotation,                          // NEW: Add throws annotation
      };
      self.functions_with_throws.insert(throw_map);
    }
  }

  fn register_import(&mut self, import: &ImportDecl) {
    self.import_sources.insert(import.src.value.to_string());
    for specifier in &import.specifiers {
      match specifier {
        ImportSpecifier::Default(default_spec) => {
          self
            .imported_identifiers
            .push(default_spec.local.sym.to_string());
        }
        ImportSpecifier::Named(named_spec) => {
          let imported_name = match &named_spec.imported {
            Some(imported) => match imported {
              ModuleExportName::Ident(ident) => ident.sym.to_string(),
              ModuleExportName::Str(str) => str.value.to_string(),
            },
            None => named_spec.local.sym.to_string(),
          };
          self.imported_identifiers.push(imported_name);
        }
        ImportSpecifier::Namespace(namespace_spec) => {
          self
            .imported_identifiers
            .push(namespace_spec.local.sym.to_string());
        }
      }
    }
  }
}

// --------- ThrowAnalyzer Visitor implementation ---------
// `ThrowAnalyzer` uses the Visitor pattern to traverse the AST of JavaScript or TypeScript code.
// Its primary goal is to identify functions that throw exceptions and record their context.
// It also records the usage of imported identifiers to help identify the context of function calls.

impl<'throwfinder_settings> Visit for ThrowAnalyzer<'throwfinder_settings> {
  fn visit_call_expr(&mut self, call: &CallExpr) {
    if let Callee::Expr(expr) = &call.callee {
      match &**expr {
        Expr::Member(member_expr) => {
          // if let Expr::Ident(object_ident) = &*member_expr.obj {
          //   self.current_class_name = Some(object_ident.sym.to_string());
          // }

          if let MemberProp::Ident(method_ident) = &member_expr.prop {
            self.current_method_name = Some(method_ident.sym.to_string());
          }

          for arg in &call.args {
            self.function_name_stack.push(
              self
                .current_method_name
                .clone()
                .unwrap_or_else(|| "<anonymous>".to_string()),
            );
            if let Expr::Arrow(arrow_expr) = &*arg.expr {
              self.check_arrow_function_for_throws(arrow_expr);
              self.visit_arrow_expr(arrow_expr)
            }
            if let Expr::Fn(fn_expr) = &*arg.expr {
              self.check_function_for_throws(&fn_expr.function);
              self.visit_function(&fn_expr.function)
            }
            self.function_name_stack.pop();
          }
        }

        Expr::Ident(ident) => {
          let called_function_name = ident.sym.to_string();
          for arg in &call.args {
            self.function_name_stack.push(called_function_name.clone());
            if let Expr::Arrow(arrow_expr) = &*arg.expr {
              self.check_arrow_function_for_throws(arrow_expr);
              self.visit_arrow_expr(arrow_expr);
            }
            if let Expr::Fn(fn_expr) = &*arg.expr {
              self.check_function_for_throws(&fn_expr.function);
              self.visit_function(&fn_expr.function);
            }
            self.function_name_stack.pop();
          }
        }

        Expr::Arrow(arrow_expr) => {
          let mut throw_finder = ThrowFinder::new(&self.throwfinder_settings, self.comments.clone());
          throw_finder.visit_arrow_expr(arrow_expr);
          if !throw_finder.throw_spans.is_empty() {
            let throws_annotation = self.extract_throws_annotation(arrow_expr.span);
            let throw_map = ThrowMap {
              throw_spans: throw_finder.throw_spans,
              throw_statement: arrow_expr.span,
              function_or_method_name: self
                .function_name_stack
                .last()
                .cloned()
                .unwrap_or_else(|| "<anonymous>".to_string()),
              class_name: None,
              id: format!(
                "{}-{}",
                self
                  .current_class_name
                  .clone()
                  .unwrap_or_else(|| "NOT_SET".to_string()),
                self
                  .function_name_stack
                  .last()
                  .cloned()
                  .unwrap_or_else(|| "<anonymous>".to_string())
              ),
              throw_details: throw_finder.throw_details,
              throws_annotation,
            };
            self.functions_with_throws.insert(throw_map);
          }
        }
        _ => {}
      }
    }
  }

  fn visit_fn_decl(&mut self, fn_decl: &FnDecl) {
    let function_name = fn_decl.ident.sym.to_string();
    self.function_name_stack.push(function_name);

    swc_ecma_visit::visit_fn_decl(self, fn_decl);

    self.function_name_stack.pop();
  }

  fn visit_object_lit(&mut self, object_lit: &ObjectLit) {
    // Iterate over the properties of the object literal
    for prop in &object_lit.props {
      match prop {
        // Check for method properties (e.g., someImportedThrow: () => { ... })
        PropOrSpread::Prop(prop) => {
          if let Prop::Method(method_prop) = &**prop {
            if let Some(method_name) = &method_prop.key.as_ident() {
              let method_name: String = method_name.sym.to_string();

              self.function_name_stack.push(method_name.clone());

              let mut throw_finder =
                ThrowFinder::new(&self.throwfinder_settings, self.comments.clone());
              throw_finder.visit_function(&method_prop.function);

              if !throw_finder.throw_spans.is_empty() {
                let throws_annotation = self.extract_throws_annotation(method_prop.function.span);
                let throw_map = ThrowMap {
                  throw_spans: throw_finder.throw_spans,
                  throw_statement: method_prop.function.span,
                  function_or_method_name: method_name.clone(),
                  class_name: self.current_class_name.clone(),
                  id: format!(
                    "{}-{}",
                    self
                      .current_class_name
                      .clone()
                      .unwrap_or_else(|| "NOT_SET".to_string()),
                    method_name
                  ),
                  throw_details: throw_finder.throw_details,
                  throws_annotation,
                };
                self.functions_with_throws.insert(throw_map);
              }

              self.function_name_stack.pop();
            }
          }
          if let Prop::KeyValue(key_value_prop) = &**prop {
            match &*key_value_prop.value {
              Expr::Fn(fn_expr) => {
                let mut throw_finder =
                  ThrowFinder::new(&self.throwfinder_settings, self.comments.clone());
                throw_finder.visit_function(&fn_expr.function);
                let function_name = prop_name_to_string(&key_value_prop.key);

                if !throw_finder.throw_spans.is_empty() {
                  let throws_annotation = self.extract_throws_annotation(fn_expr.function.span);
                  let throw_map = ThrowMap {
                    throw_spans: throw_finder.throw_spans,
                    throw_statement: fn_expr.function.span,
                    function_or_method_name: function_name.clone(),
                    class_name: self.current_class_name.clone(),
                    id: format!(
                      "{}-{}",
                      self
                        .current_class_name
                        .clone()
                        .unwrap_or_else(|| "NOT_SET".to_string()),
                      function_name
                    ),
                    throw_details: throw_finder.throw_details,
                    throws_annotation,
                  };
                  self.functions_with_throws.insert(throw_map);
                }
              }
              Expr::Arrow(arrow_expr) => {
                let mut throw_finder =
                  ThrowFinder::new(&self.throwfinder_settings, self.comments.clone());
                throw_finder.visit_arrow_expr(arrow_expr);
                let function_name = prop_name_to_string(&key_value_prop.key);

                if !throw_finder.throw_spans.is_empty() {
                  let throws_annotation = self.extract_throws_annotation(arrow_expr.span);
                  let throw_map = ThrowMap {
                    throw_spans: throw_finder.throw_spans,
                    throw_statement: arrow_expr.span,
                    function_or_method_name: function_name.clone(),
                    class_name: self.current_class_name.clone(),
                    id: format!(
                      "{}-{}",
                      self
                        .current_class_name
                        .clone()
                        .unwrap_or_else(|| "NOT_SET".to_string()),
                      function_name
                    ),
                    throw_details: throw_finder.throw_details,
                    throws_annotation,
                  };
                  self.functions_with_throws.insert(throw_map);
                }
              }
              _ => {}
            }
          }
        }
        _ => {}
      }
    }
    swc_ecma_visit::visit_object_lit(self, object_lit);
  }

  fn visit_var_declarator(&mut self, declarator: &VarDeclarator) {
    if let Some(ident) = &declarator.name.as_ident() {
      if let Some(init) = &declarator.init {
        let function_name = ident.sym.to_string();
        let throwfinder_settings_clone = self.throwfinder_settings.clone();
        let mut throw_finder = ThrowFinder::new(&throwfinder_settings_clone, self.comments.clone());

        // Check if the init is a function expression or arrow function
        if let Expr::Fn(fn_expr) = &**init {
          self.function_name_stack.push(function_name.clone());
          throw_finder.visit_function(&fn_expr.function);
          self.function_name_stack.pop();
        } else if let Expr::Arrow(arrow_expr) = &**init {
          self.function_name_stack.push(function_name.clone());
          throw_finder.visit_arrow_expr(arrow_expr);

          self.function_name_stack.pop();
        }

        if let Expr::Object(object_expr) = &**init {
          self.current_class_name = Some(function_name.clone());
          self.visit_object_lit(object_expr);
          self.current_class_name = None;
        }

        if !throw_finder.throw_spans.is_empty() {
          let throws_annotation = self.extract_throws_annotation(declarator.span);
          let throw_map = ThrowMap {
            throw_spans: throw_finder.throw_spans,
            throw_statement: declarator.span,
            function_or_method_name: function_name.clone(),
            class_name: self.current_class_name.clone(),
            id: format!(
              "{}-{}",
              self
                .current_class_name
                .clone()
                .unwrap_or_else(|| "NOT_SET".to_string()),
              function_name
            ),
            throw_details: throw_finder.throw_details,
            throws_annotation,
          };
          self.functions_with_throws.insert(throw_map);
        }
      }
    }
    swc_ecma_visit::visit_var_declarator(self, declarator);
  }
  fn visit_assign_expr(&mut self, assign_expr: &AssignExpr) {
    if let PatOrExpr::Expr(expr) = &assign_expr.left {
      if let Expr::Ident(ident) = &**expr {
        if matches!(&*assign_expr.right, Expr::Fn(_) | Expr::Arrow(_)) {
          let function_name = ident.sym.to_string();
          self.function_name_stack.push(function_name);
        }
      }
    }

    swc_ecma_visit::visit_assign_expr(self, assign_expr);

    if let PatOrExpr::Expr(expr) = &assign_expr.left {
      if let Expr::Ident(_) = &**expr {
        if matches!(&*assign_expr.right, Expr::Fn(_) | Expr::Arrow(_)) {
          self.function_name_stack.pop();
        }
      }
    }
  }

  fn visit_import_decl(&mut self, import: &ImportDecl) {
    self.register_import(import);
    self.import_sources.insert(import.src.value.to_string());
    swc_ecma_visit::visit_import_decl(self, import);
  }

  fn visit_function(&mut self, function: &Function) {
    if let Some(block_stmt) = &function.body {
      for stmt in &block_stmt.stmts {
        self.visit_stmt(stmt);
      }
    }
    self.check_function_for_throws(function);
    swc_ecma_visit::visit_function(self, function);
  }

  fn visit_arrow_expr(&mut self, arrow_expr: &ArrowExpr) {
    match &*arrow_expr.body {
      BlockStmtOrExpr::BlockStmt(block_stmt) => {
        for stmt in &block_stmt.stmts {
          self.visit_stmt(stmt);
        }
      }
      BlockStmtOrExpr::Expr(expr) => {
        if let Expr::Call(call_expr) = &**expr {
          self.visit_call_expr(call_expr);
        } else {
          // use default implementation for other kinds of expressions (for now)
          self.visit_expr(expr);
        }
      }
    }
    swc_ecma_visit::visit_arrow_expr(self, arrow_expr);
  }

  fn visit_stmt(&mut self, stmt: &Stmt) {
    match stmt {
      Stmt::Expr(expr_stmt) => {
        self.visit_expr(&expr_stmt.expr);
      }
      Stmt::Block(block_stmt) => {
        for stmt in &block_stmt.stmts {
          self.visit_stmt(stmt);
        }
      }
      Stmt::If(if_stmt) => {
        self.visit_expr(&if_stmt.test);
        self.visit_stmt(&if_stmt.cons);
        if let Some(alt) = &if_stmt.alt {
          self.visit_stmt(alt);
        }
      }
      _ => {
        // For other kinds of statements, we continue with the default implementation (for now)
        swc_ecma_visit::visit_stmt(self, stmt);
      }
    }
  }

  fn visit_expr(&mut self, expr: &Expr) {
    if let Expr::Call(call_expr) = expr {
      self.visit_call_expr(call_expr)
    }
    swc_ecma_visit::visit_expr(self, expr);
  }

  fn visit_constructor(&mut self, constructor: &Constructor) {
    self.current_method_name = Some("<constructor>".to_string());
    self.check_constructor_for_throws(constructor);
    if let Some(constructor) = &constructor.body {
      for stmt in &constructor.stmts {
        self.visit_stmt(stmt);
      }
    }
    swc_ecma_visit::visit_constructor(self, constructor);
    self.current_method_name = None;
  }

  fn visit_class_method(&mut self, class_method: &ClassMethod) {
    if let Some(method_name) = &class_method.key.as_ident() {
      let method_name = method_name.sym.to_string();

      self.function_name_stack.push(method_name.clone());

      let mut throw_finder = ThrowFinder::new(&self.throwfinder_settings, self.comments.clone());
      throw_finder.visit_class_method(class_method);

      if !throw_finder.throw_spans.is_empty() {
        let throws_annotation = self.extract_throws_annotation(class_method.span);
        let throw_map = ThrowMap {
          throw_spans: throw_finder.throw_spans,
          throw_statement: class_method.span,
          function_or_method_name: method_name.clone(),
          class_name: self.current_class_name.clone(),
          id: format!(
            "{}-{}",
            self
              .current_class_name
              .clone()
              .unwrap_or_else(|| "NOT_SET".to_string()),
            method_name
          ),
          throw_details: throw_finder.throw_details,
          throws_annotation,
        };
        self.functions_with_throws.insert(throw_map);
      }

      self.function_name_stack.pop();
    }

    self.function_name_stack.pop();

    swc_ecma_visit::visit_class_method(self, class_method);
  }

  fn visit_class_decl(&mut self, class_decl: &ClassDecl) {
    self.current_class_name = Some(class_decl.ident.sym.to_string());
    self.visit_class(&class_decl.class);
    self.current_class_name = None;
  }

  fn visit_export_decl(&mut self, export_decl: &ExportDecl) {
    if let Decl::Class(class_decl) = &export_decl.decl {
      self.current_class_name = Some(class_decl.ident.sym.to_string());
      self.visit_class(&class_decl.class);
      self.current_class_name = None;
    }
    // else if let Decl::Var(var_decl) = &export_decl.decl {
    //   for declar in &var_decl.decls {
    //     if let Some(ident) = &declar.name.as_ident() {
    //       self.current_class_name = Some(ident.sym.to_string());
    //     }
    //     self.visit_var_declarator(&declar);
    //     self.current_class_name = None;
    //   }

    // }
    else {
      swc_ecma_visit::visit_export_decl(self, export_decl);
    }
  }
}
