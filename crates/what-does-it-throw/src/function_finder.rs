extern crate swc_common;
extern crate swc_ecma_ast;
extern crate swc_ecma_visit;

use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use swc_ecma_ast::{
  AssignExpr, ClassDecl, ClassMethod, Constructor, Decl,
  ExportDecl, FnDecl, ObjectLit, PatOrExpr, Prop, PropName, PropOrSpread,
  VarDeclarator,
};

use self::swc_common::{comments::Comments, sync::Lrc, Span, BytePos, Spanned};
use self::swc_ecma_ast::Expr;
use self::swc_ecma_visit::Visit;

use crate::throw_finder::ThrowsAnnotation;

fn prop_name_to_string(prop_name: &PropName) -> String {
  match prop_name {
    PropName::Ident(ident) => ident.sym.to_string(),
    PropName::Str(str_) => str_.value.to_string(),
    PropName::Num(num) => num.value.to_string(),
    _ => "anonymous".to_string(),
  }
}

#[derive(Clone, Debug)]
pub enum FunctionType {
  Declaration,    // function foo() {}
  Arrow,         // const foo = () => {}
  Method,        // class { foo() {} }
  Constructor,   // class { constructor() {} }
  ObjectMethod,  // { foo() {} }
  ObjectProperty, // { foo: function() {} } or { foo: () => {} }
}

#[derive(Clone, Debug)]
pub struct FunctionMap {
  pub span: Span,
  pub name: String,
  pub class_name: Option<String>,
  pub id: String,
  pub throws_annotation: Option<ThrowsAnnotation>,
  pub function_type: FunctionType,
}

impl PartialEq for FunctionMap {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && self.span == other.span
  }
}

impl Eq for FunctionMap {}

impl Hash for FunctionMap {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.id.hash(state);
    self.span.lo.hash(state);
    self.span.hi.hash(state);
  }
}

pub struct FunctionFinder {
  pub functions: HashSet<FunctionMap>,
  pub comments: Lrc<dyn Comments>,
  pub function_name_stack: Vec<String>,
  pub current_class_name: Option<String>,
  pub class_name_stack: Vec<Option<String>>,
  pub current_method_name: Option<String>,
  // Map of function id -> per-parameter allowed throws (by index)
  pub param_throws: std::collections::HashMap<String, Vec<Vec<String>>>,
  // Optional map of callback typedef/callback names -> their @throws annotation types
  pub callback_type_throws: std::collections::HashMap<String, Vec<String>>,
}

impl FunctionFinder {
  pub fn new(comments: Lrc<dyn Comments>) -> Self {
    Self {
      functions: HashSet::new(),
      comments,
      function_name_stack: Vec::new(),
      current_class_name: None,
      class_name_stack: Vec::new(),
      current_method_name: None,
      param_throws: std::collections::HashMap::new(),
      callback_type_throws: std::collections::HashMap::new(),
    }
  }

  /// Provide typedef/@callback throws information for mapping @param {Type} to allowed throws
  pub fn with_callback_types(mut self, callback_type_throws: std::collections::HashMap<String, Vec<String>>) -> Self {
    self.callback_type_throws = callback_type_throws;
    self
  }

  fn extract_throws_annotation(&self, function_span: Span) -> Option<ThrowsAnnotation> {
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
    for offset in 1..100 {
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
      
      if search_pos.0 == 0 {
        break;
      }
    }

    #[cfg(debug_assertions)]
    eprintln!("   ❌ No throws annotation found for function span {:?}", function_span);
    
    None
  }

  fn parse_throws_comment(&self, comment_text: &str) -> Option<ThrowsAnnotation> {
    let text = comment_text.trim();
    let mut error_types: std::collections::HashSet<String> = std::collections::HashSet::new();

    let lines: Vec<&str> = text.lines()
      .map(|line| line.trim().trim_start_matches('*').trim())
      .collect();

    for line in &lines {
      if line.to_lowercase().contains("@throws") {
        if let Some(throws_pos) = line.to_lowercase().find("@throws") {
          let after_throws = &line[throws_pos + 7..].trim();

          // Only handle @throws {Type} syntax
          if let Some(start_brace) = after_throws.find('{') {
            if let Some(end_brace) = after_throws.find('}') {
              let type_name = &after_throws[start_brace + 1..end_brace].trim();
              if !type_name.is_empty() {
                error_types.insert(type_name.to_string());
              }
            }
          }
        }
      }
    }

    if !error_types.is_empty() {
      Some(ThrowsAnnotation { 
        error_types: error_types.into_iter().collect(),
        is_documented: true 
      })
    } else {
      None
    }
  }

  /// Extracts allowed @throws types for a parameter by scanning comments around its span
  fn extract_param_throws_types(&self, span: Span) -> Vec<String> {
    let mut types: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Helper to parse comments collection
    let mut parse_comments = |comments: Option<&[swc_common::comments::Comment]>| {
      if let Some(comments) = comments {
        for comment in comments {
          if let Some(annotation) = self.parse_throws_comment(&comment.text) {
            for t in annotation.error_types {
              types.insert(t);
            }
          }
        }
      }
    };

    // Check trailing comments at end of the param
    parse_comments(self.comments.get_trailing(span.hi()).as_deref());
    // Check leading comments at end position and a small window around it
    for offset in 0..=50u32 {
      let pos = BytePos(span.hi().0.saturating_add(offset));
      parse_comments(self.comments.get_leading(pos).as_deref());
      if offset > 0 {
        let pos_back = BytePos(span.hi().0.saturating_sub(offset));
        parse_comments(self.comments.get_leading(pos_back).as_deref());
        parse_comments(self.comments.get_trailing(pos_back).as_deref());
      }
    }

    types.into_iter().collect()
  }

  /// Extract per-parameter throws from JSDoc @param {Type} name by looking up callback typedef throws
  fn extract_param_throws_from_jsdoc(&self, function_span: Span, param_names: &[String]) -> Vec<Vec<String>> {
    let mut result: Vec<Vec<String>> = vec![Vec::new(); param_names.len()];

    // Read leading comments for the function
    if let Some(comments) = self.comments.get_leading(function_span.lo()) {
      for comment in comments {
        let text = comment.text.trim();
        let lines: Vec<&str> = text.lines().map(|l| l.trim().trim_start_matches('*').trim()).collect();
        for line in &lines {
          // Match lines like: @param {TypeName} paramName - desc
          if line.starts_with("@param") {
            // Find type in braces
            let type_name = if let Some(start) = line.find('{') { if let Some(end) = line[start+1..].find('}') { Some(line[start+1..start+1+end].trim().to_string()) } else { None } } else { None };
            if let Some(type_name) = type_name {
              // Extract the parameter name following the type
              let after_brace = &line.split('}').nth(1).unwrap_or("").trim();
              let param_token = after_brace.split_whitespace().next().unwrap_or("");
              // Find index of this param
              if !param_token.is_empty() {
                if let Some(idx) = param_names.iter().position(|n| n == param_token) {
                  if let Some(throws) = self.callback_type_throws.get(&type_name) {
                    result[idx] = throws.clone();
                  }
                }
              }
            }
          }
        }
      }
    }

    result
  }

  fn register_function(
    &mut self,
    span: Span,
    name: String,
    function_type: FunctionType,
  ) {
    let throws_annotation = self.extract_throws_annotation(span);
    
    let function_map = FunctionMap {
      span,
      name: name.clone(),
      class_name: self.current_class_name.clone(),
      id: format!(
        "{}-{}",
        self.current_class_name.clone().unwrap_or_else(|| "NOT_SET".to_string()),
        name
      ),
      throws_annotation,
      function_type,
    };

    #[cfg(debug_assertions)]
    eprintln!("🔍 Registered function: {} (id: {}, type: {:?})", name, function_map.id, function_map.function_type);

    self.functions.insert(function_map);
  }
}

impl Visit for FunctionFinder {
  fn visit_fn_decl(&mut self, fn_decl: &FnDecl) {
    let function_name = fn_decl.ident.sym.to_string();
    self.function_name_stack.push(function_name.clone());

    // Map typedef/@callback throws into param_throws via JSDoc @param typing
    let param_names: Vec<String> = fn_decl.function.params.iter().filter_map(|p| p.pat.as_ident().map(|i| i.id.sym.to_string())).collect();
    if !param_names.is_empty() && !self.callback_type_throws.is_empty() {
      let per_param = self.extract_param_throws_from_jsdoc(fn_decl.function.span, &param_names);
      if per_param.iter().any(|v| !v.is_empty()) {
        let function_id = format!("{}-{}", self.current_class_name.clone().unwrap_or_else(|| "NOT_SET".to_string()), function_name.clone());
        self.param_throws.insert(function_id, per_param);
      }
    }

    self.register_function(fn_decl.function.span, function_name, FunctionType::Declaration);

    swc_ecma_visit::visit_fn_decl(self, fn_decl);
    self.function_name_stack.pop();
  }

  fn visit_var_declarator(&mut self, declarator: &VarDeclarator) {
    if let Some(ident) = &declarator.name.as_ident() {
      if let Some(init) = &declarator.init {
        let function_name = ident.sym.to_string();

        match &**init {
          Expr::Fn(fn_expr) => {
            self.function_name_stack.push(function_name.clone());
            // Param typedef mapping
            let param_names: Vec<String> = fn_expr.function.params.iter().filter_map(|p| p.pat.as_ident().map(|i| i.id.sym.to_string())).collect();
            if !param_names.is_empty() && !self.callback_type_throws.is_empty() {
              let per_param = self.extract_param_throws_from_jsdoc(fn_expr.function.span, &param_names);
              if per_param.iter().any(|v| !v.is_empty()) {
                let function_id = format!(
                  "{}-{}",
                  self.current_class_name.clone().unwrap_or_else(|| "NOT_SET".to_string()),
                  function_name.clone()
                );
                self.param_throws.insert(function_id, per_param);
              }
            }
            self.register_function(fn_expr.function.span, function_name, FunctionType::Declaration);
            self.visit_function(&fn_expr.function);
            self.function_name_stack.pop();
            // Don't call default visitor as we handled the function
            return;
          }
          Expr::Arrow(arrow_expr) => {
            self.function_name_stack.push(function_name.clone());
            // Param typedef mapping for arrow
            let param_names: Vec<String> = arrow_expr.params.iter().filter_map(|p| p.as_ident().map(|i| i.id.sym.to_string())).collect();
            if !param_names.is_empty() && !self.callback_type_throws.is_empty() {
              let per_param = self.extract_param_throws_from_jsdoc(arrow_expr.span, &param_names);
              if per_param.iter().any(|v| !v.is_empty()) {
                let function_id = format!(
                  "{}-{}",
                  self.current_class_name.clone().unwrap_or_else(|| "NOT_SET".to_string()),
                  function_name.clone()
                );
                self.param_throws.insert(function_id, per_param);
              }
            }
            self.register_function(arrow_expr.span, function_name, FunctionType::Arrow);
            // Capture per-parameter @throws annotations for callbacks
            let mut per_param: Vec<Vec<String>> = Vec::new();
            for param in &arrow_expr.params {
              let types = self.extract_param_throws_types(param.span());
              per_param.push(types);
            }
            let function_id = format!(
              "{}-{}",
              self.current_class_name.clone().unwrap_or_else(|| "NOT_SET".to_string()),
              self.function_name_stack.last().cloned().unwrap_or_else(|| "<anonymous>".to_string())
            );
            if !per_param.is_empty() {
              self.param_throws.insert(function_id, per_param);
            }
            self.visit_arrow_expr(arrow_expr);
            self.function_name_stack.pop();
            // Don't call default visitor as we handled the arrow function
            return;
          }
          Expr::Object(object_expr) => {
            // Push current class name onto stack and set new one
            self.class_name_stack.push(self.current_class_name.clone());
            self.current_class_name = Some(function_name.clone());
            self.visit_object_lit(object_expr);
            // Restore previous class name from stack
            self.current_class_name = self.class_name_stack.pop().unwrap_or(None);
            // Don't call default visitor as we handled the object
            return;
          }
          _ => {}
        }
      }
    }
    swc_ecma_visit::visit_var_declarator(self, declarator);
  }

  fn visit_assign_expr(&mut self, assign_expr: &AssignExpr) {
    let function_name = match &assign_expr.left {
      PatOrExpr::Expr(expr) => {
        if let Expr::Ident(ident) = &**expr {
          Some(ident.sym.to_string())
        } else {
          None
        }
      }
      PatOrExpr::Pat(pat) => {
        if let Some(ident) = pat.as_ident() {
          Some(ident.sym.to_string())
        } else {
          None
        }
      }
    };

    if let Some(function_name) = function_name {
      match &*assign_expr.right {
        Expr::Fn(fn_expr) => {
          self.function_name_stack.push(function_name.clone());
          self.register_function(fn_expr.function.span, function_name, FunctionType::Declaration);
          self.visit_function(&fn_expr.function);
          self.function_name_stack.pop();
          // Don't call default visitor for function expressions as we handled them
          swc_ecma_visit::visit_pat_or_expr(self, &assign_expr.left);
          return;
        }
        Expr::Arrow(arrow_expr) => {
          self.function_name_stack.push(function_name.clone());
          self.register_function(arrow_expr.span, function_name, FunctionType::Arrow);
          self.visit_arrow_expr(arrow_expr);
          self.function_name_stack.pop();
          // Don't call default visitor for arrow expressions as we handled them
          swc_ecma_visit::visit_pat_or_expr(self, &assign_expr.left);
          return;
        }
        _ => {}
      }
    }

    swc_ecma_visit::visit_assign_expr(self, assign_expr);
  }

  fn visit_object_lit(&mut self, object_lit: &ObjectLit) {
    for prop in &object_lit.props {
      match prop {
        PropOrSpread::Prop(prop) => {
          match &**prop {
            Prop::Method(method_prop) => {
              if let Some(method_name) = &method_prop.key.as_ident() {
                let method_name = method_name.sym.to_string();
                self.function_name_stack.push(method_name.clone());
                self.register_function(method_prop.function.span, method_name, FunctionType::ObjectMethod);
                self.visit_function(&method_prop.function);
                self.function_name_stack.pop();
              }
            }
            Prop::KeyValue(key_value_prop) => {
              let property_name = prop_name_to_string(&key_value_prop.key);
              
              match &*key_value_prop.value {
                Expr::Fn(fn_expr) => {
                  self.function_name_stack.push(property_name.clone());
                  self.register_function(fn_expr.function.span, property_name, FunctionType::ObjectProperty);
                  self.visit_function(&fn_expr.function);
                  self.function_name_stack.pop();
                }
                Expr::Arrow(arrow_expr) => {
                  self.function_name_stack.push(property_name.clone());
                  self.register_function(arrow_expr.span, property_name, FunctionType::ObjectProperty);
                  self.visit_arrow_expr(arrow_expr);
                  self.function_name_stack.pop();
                }
                _ => {
                  // For other expressions, use default visitor
                  swc_ecma_visit::visit_expr(self, &key_value_prop.value);
                }
              }
            }
            Prop::Getter(getter_prop) => {
              let getter_name = prop_name_to_string(&getter_prop.key);
              self.function_name_stack.push(getter_name.clone());
              self.register_function(getter_prop.span, getter_name, FunctionType::ObjectMethod);
              
              if let Some(body) = &getter_prop.body {
                for stmt in &body.stmts {
                  self.visit_stmt(stmt);
                }
              }
              self.function_name_stack.pop();
            }
            Prop::Setter(setter_prop) => {
              let setter_name = prop_name_to_string(&setter_prop.key);
              self.function_name_stack.push(setter_name.clone());
              self.register_function(setter_prop.span, setter_name, FunctionType::ObjectMethod);
              
              if let Some(body) = &setter_prop.body {
                for stmt in &body.stmts {
                  self.visit_stmt(stmt);
                }
              }
              self.function_name_stack.pop();
            }
            _ => {
              // For other property types, use default visitor
              swc_ecma_visit::visit_prop(self, prop);
            }
          }
        }
        _ => {
          // For spread properties, use default visitor
          swc_ecma_visit::visit_prop_or_spread(self, prop);
        }
      }
    }
    // Don't call the default visitor since we've manually handled everything
  }

  fn visit_constructor(&mut self, constructor: &Constructor) {
    self.current_method_name = Some("<constructor>".to_string());
    self.register_function(constructor.span, "<constructor>".to_string(), FunctionType::Constructor);
    swc_ecma_visit::visit_constructor(self, constructor);
    self.current_method_name = None;
  }

  fn visit_class_method(&mut self, class_method: &ClassMethod) {
    if let Some(method_name) = &class_method.key.as_ident() {
      let method_name = method_name.sym.to_string();
      self.function_name_stack.push(method_name.clone());
      self.register_function(class_method.span, method_name, FunctionType::Method);
      self.function_name_stack.pop();
    }
    swc_ecma_visit::visit_class_method(self, class_method);
  }

  fn visit_class_decl(&mut self, class_decl: &ClassDecl) {
    self.class_name_stack.push(self.current_class_name.clone());
    self.current_class_name = Some(class_decl.ident.sym.to_string());
    self.visit_class(&class_decl.class);
    self.current_class_name = self.class_name_stack.pop().unwrap_or(None);
  }

  fn visit_export_decl(&mut self, export_decl: &ExportDecl) {
    match &export_decl.decl {
      Decl::Class(class_decl) => {
        let class_name = class_decl.ident.sym.to_string();
        
        self.class_name_stack.push(self.current_class_name.clone());
        self.current_class_name = Some(class_name);
        
        // Manually visit the class members to ensure constructor gets proper class context
        for member in &class_decl.class.body {
          swc_ecma_visit::visit_class_member(self, member);
        }
        
        self.current_class_name = self.class_name_stack.pop().unwrap_or(None);
      }
      _ => {
        swc_ecma_visit::visit_export_decl(self, export_decl);
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use swc_common::{comments::SingleThreadedComments, sync::Lrc, FileName, SourceMap};
  use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsConfig};

  fn parse_code_with_comments(code: &str) -> (swc_ecma_ast::Module, Lrc<dyn Comments>) {
    let cm: Lrc<SourceMap> = Default::default();
    let comments: Lrc<dyn Comments> = Lrc::new(SingleThreadedComments::default());
    let fm = cm.new_source_file(FileName::Custom("test.ts".into()), code.into());
    
    let lexer = Lexer::new(
      Syntax::Typescript(TsConfig::default()),
      Default::default(),
      StringInput::from(&*fm),
      Some(&comments),
    );
    let mut parser = Parser::new_from(lexer);
    
    let module = match parser.parse_module() {
      Ok(module) => module,
      Err(e) => {
        eprintln!("Failed to parse module in function_finder: {:?}", e);
        // Return a default empty module on parse failure
        swc_ecma_ast::Module {
          span: swc_common::DUMMY_SP,
          body: vec![],
          shebang: None,
        }
      }
    };
    (module, comments)
  }

  fn find_functions_in_code(code: &str) -> HashSet<FunctionMap> {
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = FunctionFinder::new(comments);
    finder.visit_module(&module);
    finder.functions
  }

  #[test]
  fn test_function_declaration() {
    let code = r#"
      function testFunction() {
        return "hello";
      }
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    assert_eq!(func.name, "testFunction");
    assert_eq!(func.id, "NOT_SET-testFunction");
    assert!(matches!(func.function_type, FunctionType::Declaration));
    assert!(func.throws_annotation.is_none());
  }

  #[test]
  fn test_function_declaration_with_jsdoc() {
    let code = r#"
      /**
       * @throws {Error} when something goes wrong
       * @throws {TypeError} when input is invalid
       */
      function testFunction() {
        return "hello";
      }
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    assert_eq!(func.name, "testFunction");
    assert!(matches!(func.function_type, FunctionType::Declaration));
    
    if let Some(annotation) = &func.throws_annotation {
      assert!(annotation.is_documented);
      assert!(annotation.error_types.contains(&"Error".to_string()));
      assert!(annotation.error_types.contains(&"TypeError".to_string()));
      assert_eq!(annotation.error_types.len(), 2);
    } else {
      panic!("Expected throws annotation");
    }
  }

  #[test]
  fn test_arrow_function_const() {
    let code = r#"
      const arrowFunc = () => {
        return "hello";
      };
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    assert_eq!(func.name, "arrowFunc");
    assert_eq!(func.id, "NOT_SET-arrowFunc");
    assert!(matches!(func.function_type, FunctionType::Arrow));
  }

  #[test]
  fn test_arrow_function_with_jsdoc() {
    let code = r#"
      /**
       * @throws {ValidationError} when validation fails
       */
      const validatorFunc = () => {
        return true;
      };
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    assert_eq!(func.name, "validatorFunc");
    assert!(matches!(func.function_type, FunctionType::Arrow));
    
    if let Some(annotation) = &func.throws_annotation {
      assert!(annotation.is_documented);
      assert!(annotation.error_types.contains(&"ValidationError".to_string()));
      assert_eq!(annotation.error_types.len(), 1);
    } else {
      panic!("Expected throws annotation");
    }
  }

  #[test]
  fn test_function_expression() {
    let code = r#"
      const funcExpr = function() {
        return "hello";
      };
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    assert_eq!(func.name, "funcExpr");
    assert!(matches!(func.function_type, FunctionType::Declaration));
  }

  #[test]
  fn test_class_method() {
    let code = r#"
      class TestClass {
        /**
         * @throws {Error} when method fails
         */
        testMethod() {
          return "hello";
        }
      }
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    assert_eq!(func.name, "testMethod");
    assert_eq!(func.class_name, Some("TestClass".to_string()));
    assert_eq!(func.id, "TestClass-testMethod");
    assert!(matches!(func.function_type, FunctionType::Method));
    
    if let Some(annotation) = &func.throws_annotation {
      assert!(annotation.error_types.contains(&"Error".to_string()));
    } else {
      panic!("Expected throws annotation");
    }
  }

  #[test]
  fn test_class_constructor() {
    let code = r#"
      class TestClass {
        /**
         * @throws {TypeError} when invalid arguments
         */
        constructor() {
          // constructor logic
        }
      }
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    assert_eq!(func.name, "<constructor>");
    assert_eq!(func.class_name, Some("TestClass".to_string()));
    assert_eq!(func.id, "TestClass-<constructor>");
    assert!(matches!(func.function_type, FunctionType::Constructor));
    
    if let Some(annotation) = &func.throws_annotation {
      assert!(annotation.error_types.contains(&"TypeError".to_string()));
    } else {
      panic!("Expected throws annotation");
    }
  }

  #[test]
  fn test_object_method() {
    let code = r#"
      const obj = {
        /**
         * @throws {Error}
         */
        testMethod() {
          return "hello";
        }
      };
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    assert_eq!(func.name, "testMethod");
    assert_eq!(func.class_name, Some("obj".to_string()));
    assert_eq!(func.id, "obj-testMethod");
    assert!(matches!(func.function_type, FunctionType::ObjectMethod));
    
    if let Some(annotation) = &func.throws_annotation {
      assert!(annotation.error_types.contains(&"Error".to_string()));
    } else {
      panic!("Expected throws annotation");
    }
  }

  #[test]
  fn test_object_property_function() {
    let code = r#"
      const obj = {
        /**
         * @throws {ValidationError}
         */
        testProp: function() {
          return "hello";
        }
      };
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    assert_eq!(func.name, "testProp");
    assert_eq!(func.class_name, Some("obj".to_string()));
    assert_eq!(func.id, "obj-testProp");
    assert!(matches!(func.function_type, FunctionType::ObjectProperty));
    
    if let Some(annotation) = &func.throws_annotation {
      assert!(annotation.error_types.contains(&"ValidationError".to_string()));
    } else {
      panic!("Expected throws annotation");
    }
  }

  #[test]
  fn test_object_property_arrow() {
    let code = r#"
      const obj = {
        /**
         * @throws {TypeError}
         */
        testProp: () => {
          return "hello";
        }
      };
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    assert_eq!(func.name, "testProp");
    assert_eq!(func.class_name, Some("obj".to_string()));
    assert_eq!(func.id, "obj-testProp");
    assert!(matches!(func.function_type, FunctionType::ObjectProperty));
    
    if let Some(annotation) = &func.throws_annotation {
      assert!(annotation.error_types.contains(&"TypeError".to_string()));
    } else {
      panic!("Expected throws annotation");
    }
  }

  #[test]
  fn test_assignment_function() {
    let code = r#"
      let myFunc;
      /**
       * @throws {Error}
       */
      myFunc = function() {
        return "hello";
      };
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    assert_eq!(func.name, "myFunc");
    assert!(matches!(func.function_type, FunctionType::Declaration));
  }

  #[test]
  fn test_assignment_arrow() {
    let code = r#"
      let myFunc;
      /**
       * @throws {TypeError}
       */
      myFunc = () => {
        return "hello";
      };
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    assert_eq!(func.name, "myFunc");
    assert!(matches!(func.function_type, FunctionType::Arrow));
  }

  #[test]
  fn test_multiple_functions() {
    let code = r#"
      function func1() {}
      const func2 = () => {};
      const func3 = function() {};
      
      class TestClass {
        method1() {}
        constructor() {}
      }
      
      const obj = {
        method2() {},
        prop: function() {}
      };
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 7);
    
    let names: HashSet<String> = functions.iter().map(|f| f.name.clone()).collect();
    assert!(names.contains("func1"));
    assert!(names.contains("func2"));
    assert!(names.contains("func3"));
    assert!(names.contains("method1"));
    assert!(names.contains("<constructor>"));
    assert!(names.contains("method2"));
    assert!(names.contains("prop"));
  }

  #[test]
  fn test_exported_class() {
    let code = r#"
      export class ExportedClass {
        /**
         * @throws {Error}
         */
        exportedMethod() {
          return "hello";
        }
      }
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    assert_eq!(func.name, "exportedMethod");
    assert_eq!(func.class_name, Some("ExportedClass".to_string()));
    assert_eq!(func.id, "ExportedClass-exportedMethod");
    assert!(matches!(func.function_type, FunctionType::Method));
  }

  #[test]
  fn test_jsdoc_multiple_throws_with_braces() {
    let code = r#"
      /**
       * @throws {Error} when general error occurs
       * @throws {TypeError} when type is wrong
       * @throws {ValidationError} when validation fails
       */
      function multiThrowFunc() {
        return "hello";
      }
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    if let Some(annotation) = &func.throws_annotation {
      assert_eq!(annotation.error_types.len(), 3);
      assert!(annotation.error_types.contains(&"Error".to_string()));
      assert!(annotation.error_types.contains(&"TypeError".to_string()));
      assert!(annotation.error_types.contains(&"ValidationError".to_string()));
    } else {
      panic!("Expected throws annotation");
    }
  }



  #[test]
  fn test_no_jsdoc_annotation() {
    let code = r#"
      // Regular comment
      function noAnnotationFunc() {
        return "hello";
      }
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    assert!(func.throws_annotation.is_none());
  }

  #[test]
  fn test_nested_functions() {
    let code = r#"
      function outerFunc() {
        function innerFunc() {
          return "inner";
        }
        
        const arrowInner = () => {
          return "arrow inner";
        };
        
        return "outer";
      }
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 3);
    
    let names: HashSet<String> = functions.iter().map(|f| f.name.clone()).collect();
    assert!(names.contains("outerFunc"));
    assert!(names.contains("innerFunc"));
    assert!(names.contains("arrowInner"));
  }

  #[test]
  fn test_edge_case_anonymous_functions() {
    let code = r#"
      const obj = {
        123: function() {
          return "numeric key";
        },
        "string-key": () => {
          return "string key";
        }
      };
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 2);
    
    let names: HashSet<String> = functions.iter().map(|f| f.name.clone()).collect();
    assert!(names.contains("123"));
    assert!(names.contains("string-key"));
  }

  #[test]
  fn test_jsdoc_edge_cases() {
    let code = r#"
      /**
       * Some description
       * @param {string} input - The input parameter
       * @throws {TypeError} - Input must be string
       * @returns {string} - The processed result
       * @throws {Error} - General error case
       */
      function edgeCaseFunc() {
        return "hello";
      }
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    if let Some(annotation) = &func.throws_annotation {
      assert_eq!(annotation.error_types.len(), 2);
      assert!(annotation.error_types.contains(&"TypeError".to_string()));
      assert!(annotation.error_types.contains(&"Error".to_string()));
    } else {
      panic!("Expected throws annotation");
    }
  }

  #[test]
  fn test_duplicate_error_types_deduplication() {
    let code = r#"
      /**
       * @throws {Error} first occurrence
       * @throws {TypeError} type error
       * @throws {Error} second occurrence (should be deduplicated)
       */
      function duplicateThrowFunc() {
        return "hello";
      }
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    if let Some(annotation) = &func.throws_annotation {
      assert_eq!(annotation.error_types.len(), 2); // Should be deduplicated
      assert!(annotation.error_types.contains(&"Error".to_string()));
      assert!(annotation.error_types.contains(&"TypeError".to_string()));
    } else {
      panic!("Expected throws annotation");
    }
  }

  #[test]
  fn test_complex_nested_class_structure() {
    let code = r#"
      export class OuterClass {
        /**
         * @throws {Error}
         */
        outerMethod() {
          const innerObj = {
            /**
             * @throws {TypeError}
             */
            innerMethod() {
              return "nested";
            },
            
            /**
             * @throws {ValidationError}
             */
            innerProp: function() {
              return "nested prop";
            }
          };
          
          return innerObj;
        }
        
        /**
         * @throws {RangeError}
         */
        constructor() {
          // constructor logic
        }
      }
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 4);
    
    // Find each function by name and verify
    let outer_method = functions.iter().find(|f| f.name == "outerMethod").unwrap();
    assert_eq!(outer_method.class_name, Some("OuterClass".to_string()));
    assert!(matches!(outer_method.function_type, FunctionType::Method));
    
    let constructor = functions.iter().find(|f| f.name == "<constructor>").unwrap();
    assert_eq!(constructor.class_name, Some("OuterClass".to_string()));
    assert!(matches!(constructor.function_type, FunctionType::Constructor));
    
    let inner_method = functions.iter().find(|f| f.name == "innerMethod").unwrap();
    assert_eq!(inner_method.class_name, Some("innerObj".to_string()));
    assert!(matches!(inner_method.function_type, FunctionType::ObjectMethod));
    
    let inner_prop = functions.iter().find(|f| f.name == "innerProp").unwrap();
    assert_eq!(inner_prop.class_name, Some("innerObj".to_string()));
    assert!(matches!(inner_prop.function_type, FunctionType::ObjectProperty));
  }

  #[test]
  fn test_function_ids_uniqueness() {
    let code = r#"
      function globalFunc() {}
      
      class TestClass {
        globalFunc() {} // Same name, different context
      }
      
      const obj = {
        globalFunc() {} // Same name, different context
      };
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 3);
    
    let ids: HashSet<String> = functions.iter().map(|f| f.id.clone()).collect();
    assert_eq!(ids.len(), 3); // All IDs should be unique
    
    assert!(ids.contains("NOT_SET-globalFunc"));
    assert!(ids.contains("TestClass-globalFunc"));
    assert!(ids.contains("obj-globalFunc"));
  }

  #[test]
  fn test_malformed_jsdoc_throws() {
    let code = r#"
      /**
       * @throws {TypeError incomplete brace
       * @throws missing type
       * @throws {} empty braces
       * @throws {ValidType} - this should work
       */
      function malformedJSDocFunc() {
        return "hello";
      }
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 1);
    
    let func = functions.iter().next().unwrap();
    if let Some(annotation) = &func.throws_annotation {
      // Should only parse the valid one
      assert_eq!(annotation.error_types.len(), 1);
      assert!(annotation.error_types.contains(&"ValidType".to_string()));
    } else {
      panic!("Expected throws annotation");
    }
  }

  #[test]
  fn test_typescript_specific_syntax() {
    let code = r#"
      interface ITest {
        testMethod(): void;
      }
      
      class TypeScriptClass implements ITest {
        /**
         * @throws {Error}
         */
        testMethod(): void {
          // implementation
        }
        
        /**
         * @throws {TypeError}
         */
        private privateMethod(): string {
          return "private";
        }
        
        /**
         * @throws {ValidationError}
         */
        static staticMethod(): number {
          return 42;
        }
      }
    "#;
    
    let functions = find_functions_in_code(code);
    assert_eq!(functions.len(), 3);
    
    let names: HashSet<String> = functions.iter().map(|f| f.name.clone()).collect();
    assert!(names.contains("testMethod"));
    assert!(names.contains("privateMethod"));
    assert!(names.contains("staticMethod"));
    
    // Verify all are associated with the class
    for func in &functions {
      assert_eq!(func.class_name, Some("TypeScriptClass".to_string()));
      assert!(matches!(func.function_type, FunctionType::Method));
      assert!(func.throws_annotation.is_some());
    }
  }


} 