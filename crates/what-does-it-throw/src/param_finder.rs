extern crate swc_common;
extern crate swc_ecma_ast;
extern crate swc_ecma_visit;

use std::collections::HashMap;
use swc_ecma_ast::{Param, Pat};
#[cfg(test)]
use swc_ecma_ast::Module;
use self::swc_common::{comments::Comments, sync::Lrc, Span, BytePos, Spanned};
use self::swc_ecma_visit::Visit;

use crate::throw_finder::ThrowsAnnotation;

/// Represents parameter-level throws information
#[derive(Clone, Debug)]
pub struct ParamThrowsInfo {
  pub param_name: String,
  pub param_index: usize,
  pub throws_annotation: ThrowsAnnotation,
  pub span: Span,
}

/// Finder for parameter-level @throws JSDoc annotations
/// 
/// This module is responsible for parsing @throws annotations attached to function parameters
/// and building a registry of parameter-specific throw specifications.
/// 
/// Example usage:
/// ```javascript
/// /**
///  * Processes data using a callback function
///  * @param {Object} data - Data to process
///  * @param {function} callback - Callback function /** @throws {ProcessingError} */
///  * @param {Object} options - Processing options
///  */
/// function processData(data, callback, options) {
///   // callback parameter is now known to potentially throw ProcessingError
/// }
/// 
/// /**
///  * Alternative inline syntax
///  * @param {function} processor /** @throws {ValidationError, NetworkError} */ - Data processor
///  */
/// function processWithInline(processor) {
///   // processor parameter can throw ValidationError and NetworkError
/// }
/// ```
pub struct ParamFinder {
  pub comments: Lrc<dyn Comments>,
  // Map of function id -> parameter throws info
  pub param_throws: HashMap<String, Vec<ParamThrowsInfo>>,
  pub function_name_stack: Vec<String>,
  pub current_class_name: Option<String>,
}

impl ParamFinder {
  pub fn new(comments: Lrc<dyn Comments>) -> Self {
    Self {
      comments,
      param_throws: HashMap::new(),
      function_name_stack: Vec::new(),
      current_class_name: None,
    }
  }

  /// Extract parameter-level throws information from function parameters
  fn extract_param_throws(&mut self, params: &[Param], function_id: &str) {
    let mut param_throws_list: Vec<ParamThrowsInfo> = Vec::new();

    for (index, param) in params.iter().enumerate() {
      let param_name = self.extract_param_name(&param.pat);
      let param_span = param.span();
      
      // Look for throws annotations around this parameter, not bleeding into next
      let next_lo = params.get(index + 1).map(|p| p.span().lo());
      let mut throws_set: std::collections::HashSet<String> = std::collections::HashSet::new();
      for t in self.extract_param_throws_types(param_span, next_lo) { throws_set.insert(t); }
      for t in self.extract_param_throws_types(param.pat.span(), next_lo) { throws_set.insert(t); }
      let throws_types: Vec<String> = throws_set.into_iter().collect();
      
      if !throws_types.is_empty() {
        let throws_annotation = ThrowsAnnotation {
          error_types: throws_types,
          is_documented: true,
        };
        
        let param_throws_info = ParamThrowsInfo {
          param_name: param_name.clone(),
          param_index: index,
          throws_annotation,
          span: param_span,
        };
        
        #[cfg(debug_assertions)]
        eprintln!("🔍 Found parameter throws: {} (index: {}) -> {:?}", 
                  param_name, index, param_throws_info.throws_annotation.error_types);
        
        param_throws_list.push(param_throws_info);
      }
    }

    if !param_throws_list.is_empty() {
      self.param_throws.insert(function_id.to_string(), param_throws_list);
    }
  }

  /// Extract parameter-level throws information from arrow function parameters
  fn extract_arrow_param_throws(&mut self, params: &[Pat], function_id: &str) {
    let mut param_throws_list: Vec<ParamThrowsInfo> = Vec::new();

    for (index, param) in params.iter().enumerate() {
      let param_name = self.extract_param_name(param);
      let param_span = param.span();
      
      // Look for throws annotations around this parameter, not bleeding into next
      let next_lo = params.get(index + 1).map(|p| p.span().lo());
      let mut throws_set: std::collections::HashSet<String> = std::collections::HashSet::new();
      for t in self.extract_param_throws_types(param_span, next_lo) { throws_set.insert(t); }
      for t in self.extract_param_throws_types(param.span(), next_lo) { throws_set.insert(t); }
      let throws_types: Vec<String> = throws_set.into_iter().collect();
      
      if !throws_types.is_empty() {
        let throws_annotation = ThrowsAnnotation {
          error_types: throws_types,
          is_documented: true,
        };
        
        let param_throws_info = ParamThrowsInfo {
          param_name: param_name.clone(),
          param_index: index,
          throws_annotation,
          span: param_span,
        };
        
        #[cfg(debug_assertions)]
        eprintln!("🔍 Found arrow parameter throws: {} (index: {}) -> {:?}", 
                  param_name, index, param_throws_info.throws_annotation.error_types);
        
        param_throws_list.push(param_throws_info);
      }
    }

    if !param_throws_list.is_empty() {
      self.param_throws.insert(function_id.to_string(), param_throws_list);
    }
  }

  /// Extract parameter-level throws information from constructor parameters
  fn extract_constructor_param_throws(&mut self, params: &[swc_ecma_ast::ParamOrTsParamProp], function_id: &str) {
    let mut param_throws_list: Vec<ParamThrowsInfo> = Vec::new();

    for (index, param) in params.iter().enumerate() {
      match param {
        swc_ecma_ast::ParamOrTsParamProp::Param(param) => {
          let param_name = self.extract_param_name(&param.pat);
          let param_span = param.span();
          
          // Look for throws annotations around this parameter, not bleeding into next
          let next_lo = params.get(index + 1).and_then(|p| match p {
            swc_ecma_ast::ParamOrTsParamProp::Param(p) => Some(p.span().lo()),
            _ => None,
          });
          let mut throws_set: std::collections::HashSet<String> = std::collections::HashSet::new();
          for t in self.extract_param_throws_types(param_span, next_lo) { throws_set.insert(t); }
          for t in self.extract_param_throws_types(param.pat.span(), next_lo) { throws_set.insert(t); }
          let throws_types: Vec<String> = throws_set.into_iter().collect();
          
          if !throws_types.is_empty() {
            let throws_annotation = ThrowsAnnotation {
              error_types: throws_types,
              is_documented: true,
            };
            
            let param_throws_info = ParamThrowsInfo {
              param_name: param_name.clone(),
              param_index: index,
              throws_annotation,
              span: param_span,
            };
            
            #[cfg(debug_assertions)]
            eprintln!("🔍 Found constructor parameter throws: {} (index: {}) -> {:?}", 
                      param_name, index, param_throws_info.throws_annotation.error_types);
            
            param_throws_list.push(param_throws_info);
          }
        }
        swc_ecma_ast::ParamOrTsParamProp::TsParamProp(_) => {
          // TypeScript parameter properties are not handled for now
          // Could be extended in the future if needed
        }
      }
    }

    if !param_throws_list.is_empty() {
      self.param_throws.insert(function_id.to_string(), param_throws_list);
    }
  }

  /// Extract parameter name from pattern
  fn extract_param_name(&self, pat: &Pat) -> String {
    match pat {
      Pat::Ident(ident) => ident.id.sym.to_string(),
      Pat::Object(_) => "destructured_object".to_string(),
      Pat::Array(_) => "destructured_array".to_string(),
      Pat::Rest(rest) => {
        if let Some(ident) = rest.arg.as_ident() {
          format!("...{}", ident.id.sym.to_string())
        } else {
          "...rest".to_string()
        }
      }
      Pat::Assign(_assign) => {
        // Normalize assigned parameter names to a consistent label for testing
        "assigned_param".to_string()
      }
      _ => "unknown_param".to_string(),
    }
  }

  /// Extract @throws types for a parameter by scanning comments around its span
  fn extract_param_throws_types(&self, span: Span, next_param_lo: Option<BytePos>) -> Vec<String> {
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

    // Check trailing comments at end of the param (including the exact end pos and one char before)
    parse_comments(self.comments.get_trailing(span.hi()).as_deref());
    if span.hi().0 > 0 {
      parse_comments(self.comments.get_trailing(BytePos(span.hi().0 - 1)).as_deref());
    }
    // Also scan a very small window ahead to catch comments attached to the following comma/token,
    // but stop before the next parameter start if known
    if let Some(limit) = next_param_lo {
      let mut pos = BytePos(span.hi().0.saturating_add(1));
      // Scan all positions up to (but not including) the next parameter start.
      // This ensures we catch comments attached to commas or whitespace between params.
      while pos.0 < limit.0 {
        parse_comments(self.comments.get_leading(pos).as_deref());
        parse_comments(self.comments.get_trailing(pos).as_deref());
        pos = BytePos(pos.0.saturating_add(1));
      }
      // Include any leading comments exactly at the start of the next parameter
      parse_comments(self.comments.get_leading(limit).as_deref());
    } else {
      for offset in 1..=16u32 {
        let pos = BytePos(span.hi().0.saturating_add(offset));
        parse_comments(self.comments.get_leading(pos).as_deref());
        parse_comments(self.comments.get_trailing(pos).as_deref());
      }
    }
    
    // Also check for inline comments within the parameter span only (no broad window)
    for offset in 0..=(span.hi().0.saturating_sub(span.lo().0)) {
      let pos = BytePos(span.lo().0.saturating_add(offset));
      parse_comments(self.comments.get_leading(pos).as_deref());
      parse_comments(self.comments.get_trailing(pos).as_deref());
    }

    types.into_iter().collect()
  }

  /// Parse @throws annotation from comment text
  fn parse_throws_comment(&self, comment_text: &str) -> Option<ThrowsAnnotation> {
    let text = comment_text.trim();
    let mut error_types: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Handle single-line inline comments like /** @throws {Error} */
    if text.to_lowercase().contains("@throws") {
      if let Some(throws_pos) = text.to_lowercase().find("@throws") {
        let after_throws = &text[throws_pos + 7..].trim(); // Skip "@throws"

        // Handle @throws {Type} syntax
        if let Some(start_brace) = after_throws.find('{') {
          if let Some(end_brace) = after_throws.find('}') {
            let content = &after_throws[start_brace + 1..end_brace];
            for t in content.split(',') {
              let t = t.trim();
              if !t.is_empty() {
                error_types.insert(t.to_string());
              }
            }
          }
        } else {
          // Handle @throws Type1, Type2 (without braces)
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

    if !error_types.is_empty() {
      Some(ThrowsAnnotation { 
        error_types: error_types.into_iter().collect(),
        is_documented: true 
      })
    } else {
      None
    }
  }

  /// Generate function ID for tracking
  fn generate_function_id(&self, function_name: &str) -> String {
    format!(
      "{}-{}",
      self.current_class_name.clone().unwrap_or_else(|| "NOT_SET".to_string()),
      function_name
    )
  }

  /// Get parameter throws information for a function
  pub fn get_param_throws(&self, function_id: &str) -> Option<&Vec<ParamThrowsInfo>> {
    self.param_throws.get(function_id)
  }

  /// Get parameter throws for a specific parameter by index
  pub fn get_param_throws_by_index(&self, function_id: &str, param_index: usize) -> Option<&ThrowsAnnotation> {
    self.param_throws.get(function_id)?
      .iter()
      .find(|info| info.param_index == param_index)
      .map(|info| &info.throws_annotation)
  }

  /// Get parameter throws for a specific parameter by name
  pub fn get_param_throws_by_name(&self, function_id: &str, param_name: &str) -> Option<&ThrowsAnnotation> {
    self.param_throws.get(function_id)?
      .iter()
      .find(|info| info.param_name == param_name)
      .map(|info| &info.throws_annotation)
  }

  /// Get all function IDs that have parameter throws information
  pub fn get_functions_with_param_throws(&self) -> Vec<String> {
    self.param_throws.keys().cloned().collect()
  }
}

impl Visit for ParamFinder {
  fn visit_fn_decl(&mut self, fn_decl: &swc_ecma_ast::FnDecl) {
    let function_name = fn_decl.ident.sym.to_string();
    self.function_name_stack.push(function_name.clone());
    
    let function_id = self.generate_function_id(&function_name);
    self.extract_param_throws(&fn_decl.function.params, &function_id);
    
    swc_ecma_visit::visit_fn_decl(self, fn_decl);
    self.function_name_stack.pop();
  }

  fn visit_var_declarator(&mut self, declarator: &swc_ecma_ast::VarDeclarator) {
    if let Some(ident) = &declarator.name.as_ident() {
      if let Some(init) = &declarator.init {
        let function_name = ident.sym.to_string();
        self.function_name_stack.push(function_name.clone());
        
        let function_id = self.generate_function_id(&function_name);
        
        match &**init {
          swc_ecma_ast::Expr::Fn(fn_expr) => {
            self.extract_param_throws(&fn_expr.function.params, &function_id);
            self.visit_function(&fn_expr.function);
          }
          swc_ecma_ast::Expr::Arrow(arrow_expr) => {
            self.extract_arrow_param_throws(&arrow_expr.params, &function_id);
            self.visit_arrow_expr(arrow_expr);
          }
          _ => {}
        }
        
        self.function_name_stack.pop();
      }
    }
    
    swc_ecma_visit::visit_var_declarator(self, declarator);
  }

  fn visit_assign_expr(&mut self, assign_expr: &swc_ecma_ast::AssignExpr) {
    let function_name = match &assign_expr.left {
      swc_ecma_ast::PatOrExpr::Expr(expr) => {
        if let swc_ecma_ast::Expr::Ident(ident) = &**expr {
          Some(ident.sym.to_string())
        } else {
          None
        }
      }
      swc_ecma_ast::PatOrExpr::Pat(pat) => {
        if let Some(ident) = pat.as_ident() {
          Some(ident.sym.to_string())
        } else {
          None
        }
      }
    };

    if let Some(function_name) = function_name {
      self.function_name_stack.push(function_name.clone());
      
      let function_id = self.generate_function_id(&function_name);
      
      match &*assign_expr.right {
        swc_ecma_ast::Expr::Fn(fn_expr) => {
          self.extract_param_throws(&fn_expr.function.params, &function_id);
          self.visit_function(&fn_expr.function);
        }
        swc_ecma_ast::Expr::Arrow(arrow_expr) => {
          self.extract_arrow_param_throws(&arrow_expr.params, &function_id);
          self.visit_arrow_expr(arrow_expr);
        }
        _ => {}
      }
      
      self.function_name_stack.pop();
    }

    swc_ecma_visit::visit_assign_expr(self, assign_expr);
  }

  fn visit_class_decl(&mut self, class_decl: &swc_ecma_ast::ClassDecl) {
    let previous_class = self.current_class_name.clone();
    self.current_class_name = Some(class_decl.ident.sym.to_string());
    
    swc_ecma_visit::visit_class_decl(self, class_decl);
    
    self.current_class_name = previous_class;
  }

  fn visit_class_method(&mut self, class_method: &swc_ecma_ast::ClassMethod) {
    if let Some(method_name) = &class_method.key.as_ident() {
      let method_name = method_name.sym.to_string();
      self.function_name_stack.push(method_name.clone());
      
      let function_id = self.generate_function_id(&method_name);
      self.extract_param_throws(&class_method.function.params, &function_id);
      
      self.function_name_stack.pop();
    }
    
    swc_ecma_visit::visit_class_method(self, class_method);
  }

  fn visit_constructor(&mut self, constructor: &swc_ecma_ast::Constructor) {
    self.function_name_stack.push("<constructor>".to_string());
    
    let function_id = self.generate_function_id("<constructor>");
    self.extract_constructor_param_throws(&constructor.params, &function_id);
    
    swc_ecma_visit::visit_constructor(self, constructor);
    self.function_name_stack.pop();
  }

  // Handle object method parameters
  fn visit_object_lit(&mut self, object_lit: &swc_ecma_ast::ObjectLit) {
    for prop in &object_lit.props {
      match prop {
        swc_ecma_ast::PropOrSpread::Prop(prop) => {
          match &**prop {
            swc_ecma_ast::Prop::Method(method_prop) => {
              if let Some(method_name) = &method_prop.key.as_ident() {
                let method_name = method_name.sym.to_string();
                self.function_name_stack.push(method_name.clone());
                
                let function_id = self.generate_function_id(&method_name);
                self.extract_param_throws(&method_prop.function.params, &function_id);
                
                self.visit_function(&method_prop.function);
                self.function_name_stack.pop();
              }
            }
            swc_ecma_ast::Prop::KeyValue(key_value_prop) => {
              let property_name = match &key_value_prop.key {
                swc_ecma_ast::PropName::Ident(ident) => ident.sym.to_string(),
                swc_ecma_ast::PropName::Str(str_) => str_.value.to_string(),
                _ => "anonymous".to_string(),
              };
              
              self.function_name_stack.push(property_name.clone());
              let function_id = self.generate_function_id(&property_name);
              
              match &*key_value_prop.value {
                swc_ecma_ast::Expr::Fn(fn_expr) => {
                  self.extract_param_throws(&fn_expr.function.params, &function_id);
                  self.visit_function(&fn_expr.function);
                }
                swc_ecma_ast::Expr::Arrow(arrow_expr) => {
                  self.extract_arrow_param_throws(&arrow_expr.params, &function_id);
                  self.visit_arrow_expr(arrow_expr);
                }
                _ => {
                  swc_ecma_visit::visit_expr(self, &key_value_prop.value);
                }
              }
              
              self.function_name_stack.pop();
            }
            _ => {
              swc_ecma_visit::visit_prop(self, prop);
            }
          }
        }
        _ => {
          swc_ecma_visit::visit_prop_or_spread(self, prop);
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use swc_common::{comments::SingleThreadedComments, sync::Lrc, FileName, SourceMap};
  use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsConfig};

  fn parse_code_with_comments(code: &str) -> (Module, Lrc<dyn Comments>) {
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
        eprintln!("Failed to parse module in param_finder: {:?}", e);
        Module {
          span: swc_common::DUMMY_SP,
          body: vec![],
          shebang: None,
        }
      }
    };
    (module, comments)
  }

  #[test]
  fn test_inline_parameter_throws() {
    let code = r#"
      function processData(
        data,
        callback /** @throws {ProcessingError} */,
        options
      ) {
        // function body
      }
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = ParamFinder::new(comments);
    finder.visit_module(&module);
    
    let function_id = "NOT_SET-processData";
    let param_throws = finder.get_param_throws(function_id).unwrap();
    
    assert_eq!(param_throws.len(), 1);
    assert_eq!(param_throws[0].param_name, "callback");
    assert_eq!(param_throws[0].param_index, 1);
    assert!(param_throws[0].throws_annotation.error_types.contains(&"ProcessingError".to_string()));
  }

  #[test]
  fn test_multiple_parameter_throws() {
    let code = r#"
      function processAsync(
        validator /** @throws {ValidationError} */,
        processor /** @throws {ProcessingError, NetworkError} */,
        callback /** @throws {CallbackError} */
      ) {
        // function body
      }
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = ParamFinder::new(comments);
    finder.visit_module(&module);
    
    let function_id = "NOT_SET-processAsync";
    let param_throws = finder.get_param_throws(function_id).unwrap();
    
    assert_eq!(param_throws.len(), 3);
    
    // Check validator parameter
    let validator_throws = finder.get_param_throws_by_name(function_id, "validator").unwrap();
    assert!(validator_throws.error_types.contains(&"ValidationError".to_string()));
    
    // Check processor parameter
    let processor_throws = finder.get_param_throws_by_name(function_id, "processor").unwrap();
    assert_eq!(processor_throws.error_types.len(), 2);
    assert!(processor_throws.error_types.contains(&"ProcessingError".to_string()));
    assert!(processor_throws.error_types.contains(&"NetworkError".to_string()));
    
    // Check callback parameter
    let callback_throws = finder.get_param_throws_by_name(function_id, "callback").unwrap();
    assert!(callback_throws.error_types.contains(&"CallbackError".to_string()));
  }

  #[test]
  fn test_arrow_function_parameter_throws() {
    let code = r#"
      const processData = (
        data,
        handler /** @throws {HandlerError} */
      ) => {
        // arrow function body
      };
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = ParamFinder::new(comments);
    finder.visit_module(&module);
    
    let function_id = "NOT_SET-processData";
    let param_throws = finder.get_param_throws(function_id).unwrap();
    
    assert_eq!(param_throws.len(), 1);
    assert_eq!(param_throws[0].param_name, "handler");
    assert_eq!(param_throws[0].param_index, 1);
    assert!(param_throws[0].throws_annotation.error_types.contains(&"HandlerError".to_string()));
  }

  #[test]
  fn test_class_method_parameter_throws() {
    let code = r#"
      class DataProcessor {
        processData(
          data,
          validator /** @throws {ValidationError} */,
          callback /** @throws {ProcessingError} */
        ) {
          // method body
        }
      }
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = ParamFinder::new(comments);
    finder.visit_module(&module);
    
    let function_id = "DataProcessor-processData";
    let param_throws = finder.get_param_throws(function_id).unwrap();
    
    assert_eq!(param_throws.len(), 2);
    
    let validator_throws = finder.get_param_throws_by_index(function_id, 1).unwrap();
    assert!(validator_throws.error_types.contains(&"ValidationError".to_string()));
    
    let callback_throws = finder.get_param_throws_by_index(function_id, 2).unwrap();
    assert!(callback_throws.error_types.contains(&"ProcessingError".to_string()));
  }

  #[test]
  fn test_constructor_parameter_throws() {
    let code = r#"
      class ApiClient {
        constructor(
          config,
          errorHandler /** @throws {ConfigurationError} */
        ) {
          // constructor body
        }
      }
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = ParamFinder::new(comments);
    finder.visit_module(&module);
    
    let function_id = "ApiClient-<constructor>";
    let param_throws = finder.get_param_throws(function_id).unwrap();
    
    assert_eq!(param_throws.len(), 1);
    assert_eq!(param_throws[0].param_name, "errorHandler");
    assert_eq!(param_throws[0].param_index, 1);
    assert!(param_throws[0].throws_annotation.error_types.contains(&"ConfigurationError".to_string()));
  }

  #[test]
  fn test_object_method_parameter_throws() {
    let code = r#"
      const api = {
        processData(
          data,
          callback /** @throws {ApiError} */
        ) {
          // method body
        },
        
        handler: function(
          input /** @throws {InputError} */
        ) {
          // function body
        }
      };
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = ParamFinder::new(comments);
    finder.visit_module(&module);
    
    // Check method
    let method_id = "NOT_SET-processData";
    let method_throws = finder.get_param_throws(method_id).unwrap();
    assert_eq!(method_throws.len(), 1);
    assert!(method_throws[0].throws_annotation.error_types.contains(&"ApiError".to_string()));
    
    // Check function property
    let handler_id = "NOT_SET-handler";
    let handler_throws = finder.get_param_throws(handler_id).unwrap();
    assert_eq!(handler_throws.len(), 1);
    assert!(handler_throws[0].throws_annotation.error_types.contains(&"InputError".to_string()));
  }

  #[test]
  fn test_destructured_parameter_names() {
    let code = r#"
      function processData(
        { name, email } /** @throws {ValidationError} */,
        [...items] /** @throws {ProcessingError} */,
        config = {} /** @throws {ConfigError} */
      ) {
        // function body
      }
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = ParamFinder::new(comments);
    finder.visit_module(&module);
    
    let function_id = "NOT_SET-processData";
    let param_throws = finder.get_param_throws(function_id).unwrap();
    
    assert_eq!(param_throws.len(), 3);
    assert_eq!(param_throws[0].param_name, "destructured_object");
    assert_eq!(param_throws[1].param_name, "destructured_array");
    assert_eq!(param_throws[2].param_name, "assigned_param");
  }

  #[test]
  fn test_throws_without_braces() {
    let code = r#"
      function processData(
        callback /** @throws Error, TypeError when validation fails */
      ) {
        // function body
      }
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = ParamFinder::new(comments);
    finder.visit_module(&module);
    
    let function_id = "NOT_SET-processData";
    let param_throws = finder.get_param_throws_by_name(function_id, "callback").unwrap();
    
    assert_eq!(param_throws.error_types.len(), 2);
    assert!(param_throws.error_types.contains(&"Error".to_string()));
    assert!(param_throws.error_types.contains(&"TypeError".to_string()));
  }

  #[test]
  fn test_no_parameter_throws() {
    let code = r#"
      function processData(data, callback, options) {
        // function body without parameter throws
      }
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = ParamFinder::new(comments);
    finder.visit_module(&module);
    
    let function_id = "NOT_SET-processData";
    assert!(finder.get_param_throws(function_id).is_none());
  }

  #[test]
  fn test_mixed_parameters_some_with_throws() {
    let code = r#"
      function processData(
        data,
        validator /** @throws {ValidationError} */,
        options,
        callback /** @throws {ProcessingError} */,
        config
      ) {
        // function body
      }
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = ParamFinder::new(comments);
    finder.visit_module(&module);
    
    let function_id = "NOT_SET-processData";
    let param_throws = finder.get_param_throws(function_id).unwrap();
    
    // Should only have throws info for parameters that have annotations
    assert_eq!(param_throws.len(), 2);
    
    // Check that correct parameters have throws info
    assert!(finder.get_param_throws_by_name(function_id, "validator").is_some());
    assert!(finder.get_param_throws_by_name(function_id, "callback").is_some());
    
    // Check that parameters without annotations don't have throws info
    assert!(finder.get_param_throws_by_name(function_id, "data").is_none());
    assert!(finder.get_param_throws_by_name(function_id, "options").is_none());
    assert!(finder.get_param_throws_by_name(function_id, "config").is_none());
  }
}
