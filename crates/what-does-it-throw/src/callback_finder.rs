extern crate swc_common;
extern crate swc_ecma_ast;
extern crate swc_ecma_visit;

use std::collections::HashMap;
use swc_ecma_ast::Module;
use self::swc_common::{comments::Comments, sync::Lrc, Span, Spanned};
use self::swc_ecma_visit::{Visit, VisitWith};

use crate::throw_finder::{CallbackDefinition, ThrowsAnnotation};

/// Finder for @callback JSDoc annotations
/// 
/// This module is responsible for parsing @callback JSDoc annotations and building
/// a registry of callback type definitions with their associated @throws specifications.
/// 
/// Example usage:
/// ```javascript
/// /**
///  * @callback ErrorCallback
///  * @param {Error} error - The error that occurred
///  * @throws {NetworkError} When network request fails
///  * @throws {ValidationError} When input validation fails
///  */
/// 
/// /**
///  * @param {ErrorCallback} callback - Function that may throw specific errors
///  */
/// function processAsync(callback) {
///   // callback is now known to potentially throw NetworkError and ValidationError
/// }
/// ```
pub struct CallbackFinder {
  pub comments: Lrc<dyn Comments>,
  pub callbacks: HashMap<String, CallbackDefinition>,
}

impl CallbackFinder {
  pub fn new(comments: Lrc<dyn Comments>) -> Self {
    Self {
      comments,
      callbacks: HashMap::new(),
    }
  }

  /// Parse a JSDoc comment block for @callback definitions
  fn parse_callback_comment(&mut self, comment_text: &str, span: Span) {
    let text = comment_text.trim();
    let lines: Vec<&str> = text.lines()
      .map(|line| line.trim().trim_start_matches('*').trim())
      .collect();

    let mut callback_name: Option<String> = None;
    // Aggregate all @throws across the comment block
    let mut aggregated_error_types: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in &lines {
      // Parse @callback CallbackName
      if line.to_lowercase().contains("@callback") {
        if let Some(callback_pos) = line.to_lowercase().find("@callback") {
          let after_callback = &line[callback_pos + 9..].trim(); // Skip "@callback"
          let mut parts = after_callback.split_whitespace();
          let first = parts.next().unwrap_or("");
          let has_extra = parts.next().is_some();
          if !first.is_empty() && !has_extra {
            callback_name = Some(first.to_string());
          }
        }
      }
      
      // Parse @throws annotations within the same comment block
      if line.to_lowercase().contains("@throws") {
        if let Some(annotation) = self.parse_throws_comment(line) {
          for t in annotation.error_types {
            aggregated_error_types.insert(t);
          }
        }
      }
    }

    // Register the callback definition if we found a name
    if let Some(name) = callback_name {
      // Ignore invalid names (empty or containing whitespace)
      if name.is_empty() || name.split_whitespace().count() != 1 {
        return;
      }

      let throws_annotation = if !aggregated_error_types.is_empty() {
        Some(ThrowsAnnotation { error_types: aggregated_error_types.into_iter().collect(), is_documented: true })
      } else {
        None
      };

      let callback_def = CallbackDefinition {
        name: name.clone(),
        throws_annotation,
        span,
      };
      
      #[cfg(debug_assertions)]
      eprintln!("🔍 Found @callback definition: {} with throws: {:?}", name, callback_def.throws_annotation);
      
      self.callbacks.insert(name, callback_def);
    }
  }

  /// Parse @throws annotation from a single line
  fn parse_throws_comment(&self, line: &str) -> Option<ThrowsAnnotation> {
    let mut error_types: std::collections::HashSet<String> = std::collections::HashSet::new();

    if let Some(throws_pos) = line.to_lowercase().find("@throws") {
      let after_throws = &line[throws_pos + 7..].trim(); // Skip "@throws"

      // Handle @throws {Type} syntax
      if let Some(start_brace) = after_throws.find('{') {
        if let Some(end_brace) = after_throws.find('}') {
          let content = &after_throws[start_brace + 1..end_brace];
          for t in content.split(',') {
            let t = t.trim();
            if !t.is_empty() {
              let looks_valid = t.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false) || t.ends_with("Error");
              let ident_like = t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
              if looks_valid && ident_like {
                error_types.insert(t.to_string());
              }
            }
          }
        }
      } else {
        // Handle @throws Type (without braces) - extract comma-separated types
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
          let looks_valid = error_type.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false) || error_type.ends_with("Error");
          let ident_like = error_type.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
          if looks_valid && ident_like {
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

  /// Analyze all comments in the module to find @callback definitions
  pub fn analyze_module(&mut self, module: &Module) {
    // Visit the module to trigger comment analysis
    self.visit_module(module);
    
    // Also scan all comments directly to catch top-level @callback definitions
    // that might not be attached to specific AST nodes
    self.scan_all_comments();
  }

  /// Scan all comments in the source for @callback definitions
  fn scan_all_comments(&mut self) {
    // Get all leading comments throughout the file
    // This is a simplified approach - in a real implementation, you'd want to
    // iterate through all comment positions in the source map
    
    // For now, we rely on the Visit implementation to catch most cases
    // This method can be enhanced to scan comments more comprehensively
  }

  /// Get a callback definition by name
  pub fn get_callback(&self, name: &str) -> Option<&CallbackDefinition> {
    self.callbacks.get(name)
  }

  /// Get all callback definitions
  pub fn get_all_callbacks(&self) -> &HashMap<String, CallbackDefinition> {
    &self.callbacks
  }
}

impl Visit for CallbackFinder {
  fn visit_module(&mut self, module: &Module) {
    // Check for leading comments on the module itself
    if let Some(comments) = self.comments.get_leading(module.span.lo()) {
      for comment in comments {
        self.parse_callback_comment(&comment.text, comment.span);
      }
    }
    
    // Continue visiting child nodes to catch comments attached to statements
    module.visit_children_with(self);
  }

  // We implement visit methods for various AST nodes to catch comments
  // that might be attached to different types of statements
  
  fn visit_stmt(&mut self, stmt: &swc_ecma_ast::Stmt) {
    // Check for leading comments on statements
    if let Some(comments) = self.comments.get_leading(stmt.span().lo()) {
      for comment in comments {
        self.parse_callback_comment(&comment.text, comment.span);
      }
    }
    
    stmt.visit_children_with(self);
  }

  fn visit_decl(&mut self, decl: &swc_ecma_ast::Decl) {
    // Check for leading comments on declarations
    if let Some(comments) = self.comments.get_leading(decl.span().lo()) {
      for comment in comments {
        self.parse_callback_comment(&comment.text, comment.span);
      }
    }
    
    decl.visit_children_with(self);
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
        eprintln!("Failed to parse module in callback_finder: {:?}", e);
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
  fn test_basic_callback_definition() {
    let code = r#"
      /**
       * @callback ErrorCallback
       * @throws {NetworkError} When network request fails
       * @throws {ValidationError} When input validation fails
       */
      
      function processAsync() {
        // function body
      }
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = CallbackFinder::new(comments);
    finder.analyze_module(&module);
    
    assert_eq!(finder.callbacks.len(), 1);
    
    let callback = finder.get_callback("ErrorCallback").unwrap();
    assert_eq!(callback.name, "ErrorCallback");
    
    if let Some(throws) = &callback.throws_annotation {
      assert!(throws.is_documented);
      assert_eq!(throws.error_types.len(), 2);
      assert!(throws.error_types.contains(&"NetworkError".to_string()));
      assert!(throws.error_types.contains(&"ValidationError".to_string()));
    } else {
      panic!("Expected throws annotation");
    }
  }

  #[test]
  fn test_callback_without_throws() {
    let code = r#"
      /**
       * @callback SimpleCallback
       * @param {string} data - The data to process
       */
      
      function process() {}
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = CallbackFinder::new(comments);
    finder.analyze_module(&module);
    
    assert_eq!(finder.callbacks.len(), 1);
    
    let callback = finder.get_callback("SimpleCallback").unwrap();
    assert_eq!(callback.name, "SimpleCallback");
    assert!(callback.throws_annotation.is_none());
  }

  #[test]
  fn test_multiple_callback_definitions() {
    let code = r#"
      /**
       * @callback SuccessCallback
       * @throws {ValidationError} When validation fails
       */
      
      /**
       * @callback ErrorCallback  
       * @throws {NetworkError} When network fails
       * @throws {TimeoutError} When request times out
       */
      
      function api() {}
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = CallbackFinder::new(comments);
    finder.analyze_module(&module);
    
    assert_eq!(finder.callbacks.len(), 2);
    
    let success_callback = finder.get_callback("SuccessCallback").unwrap();
    assert_eq!(success_callback.name, "SuccessCallback");
    if let Some(throws) = &success_callback.throws_annotation {
      assert_eq!(throws.error_types.len(), 1);
      assert!(throws.error_types.contains(&"ValidationError".to_string()));
    }
    
    let error_callback = finder.get_callback("ErrorCallback").unwrap();
    assert_eq!(error_callback.name, "ErrorCallback");
    if let Some(throws) = &error_callback.throws_annotation {
      assert_eq!(throws.error_types.len(), 2);
      assert!(throws.error_types.contains(&"NetworkError".to_string()));
      assert!(throws.error_types.contains(&"TimeoutError".to_string()));
    }
  }

  #[test]
  fn test_throws_without_braces() {
    let code = r#"
      /**
       * @callback LegacyCallback
       * @throws Error, TypeError when something goes wrong
       */
      
      function legacy() {}
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = CallbackFinder::new(comments);
    finder.analyze_module(&module);
    
    assert_eq!(finder.callbacks.len(), 1);
    
    let callback = finder.get_callback("LegacyCallback").unwrap();
    if let Some(throws) = &callback.throws_annotation {
      assert_eq!(throws.error_types.len(), 2);
      assert!(throws.error_types.contains(&"Error".to_string()));
      assert!(throws.error_types.contains(&"TypeError".to_string()));
    }
  }

  #[test]
  fn test_malformed_callback_definitions() {
    let code = r#"
      /**
       * @callback incomplete callback name
       * @throws {ValidError} This should work
       */
      
      /**
       * @callback
       * @throws {AnotherError} Missing callback name
       */
      
      /**
       * @callback ValidCallback
       * @throws {TypeError incomplete brace
       * @throws missing type
       * @throws {} empty braces
       * @throws {ValidType} This should work
       */
      
      function test() {}
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = CallbackFinder::new(comments);
    finder.analyze_module(&module);
    
    // Should only find ValidCallback (the one with a proper name)
    assert_eq!(finder.callbacks.len(), 1);
    
    let callback = finder.get_callback("ValidCallback").unwrap();
    if let Some(throws) = &callback.throws_annotation {
      // Should only parse the valid @throws
      assert_eq!(throws.error_types.len(), 1);
      assert!(throws.error_types.contains(&"ValidType".to_string()));
    }
  }

  #[test]
  fn test_callback_with_complex_jsdoc() {
    let code = r#"
      /**
       * Processes user data with error handling
       * @callback UserProcessor
       * @param {Object} userData - The user data to process
       * @param {string} userData.name - User's name
       * @param {string} userData.email - User's email
       * @returns {Promise<User>} Processed user object
       * @throws {ValidationError} When user data is invalid
       * @throws {DatabaseError} When database operation fails
       * @throws {AuthenticationError} When user is not authenticated
       * @example
       * // Usage example
       * processUser(userData, myProcessor);
       */
      
      function processUser() {}
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = CallbackFinder::new(comments);
    finder.analyze_module(&module);
    
    assert_eq!(finder.callbacks.len(), 1);
    
    let callback = finder.get_callback("UserProcessor").unwrap();
    assert_eq!(callback.name, "UserProcessor");
    
    if let Some(throws) = &callback.throws_annotation {
      assert!(throws.is_documented);
      assert_eq!(throws.error_types.len(), 3);
      assert!(throws.error_types.contains(&"ValidationError".to_string()));
      assert!(throws.error_types.contains(&"DatabaseError".to_string()));
      assert!(throws.error_types.contains(&"AuthenticationError".to_string()));
    } else {
      panic!("Expected throws annotation");
    }
  }
}
