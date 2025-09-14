extern crate swc_common;
extern crate swc_ecma_ast;
extern crate swc_ecma_visit;

use std::collections::HashMap;
use swc_ecma_ast::Module;
use self::swc_common::{comments::Comments, sync::Lrc, Span, Spanned};
use self::swc_ecma_visit::{Visit, VisitWith};

use crate::throw_finder::{TypedefDefinition, ThrowsAnnotation};

/// Finder for @typedef JSDoc annotations
/// 
/// This module is responsible for parsing @typedef JSDoc annotations and building
/// a registry of type definitions with their associated @throws specifications.
/// 
/// Example usage:
/// ```javascript
/// /**
///  * @typedef {function} AsyncProcessor
///  * @param {Object} data - Data to process
///  * @throws {ProcessingError} When processing fails
///  * @throws {ValidationError} When data is invalid
///  */
/// 
/// /**
///  * @typedef {Object} UserData
///  * @property {string} name - User name
///  * @property {string} email - User email
///  */
/// 
/// /**
///  * @param {AsyncProcessor} processor - Function that processes data
///  * @param {UserData} userData - User data to process
///  */
/// function processUser(processor, userData) {
///   // processor is now known to potentially throw ProcessingError and ValidationError
/// }
/// ```
pub struct TypedefFinder {
  pub comments: Lrc<dyn Comments>,
  pub typedefs: HashMap<String, TypedefDefinition>,
}

impl TypedefFinder {
  pub fn new(comments: Lrc<dyn Comments>) -> Self {
    Self {
      comments,
      typedefs: HashMap::new(),
    }
  }

  /// Parse a JSDoc comment block for @typedef definitions
  fn parse_typedef_comment(&mut self, comment_text: &str, span: Span) {
    let text = comment_text.trim();
    let lines: Vec<&str> = text.lines()
      .map(|line| line.trim().trim_start_matches('*').trim())
      .collect();

    let mut typedef_name: Option<String> = None;
    let mut is_callback: bool = false;
    let mut aggregated_error_types: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in &lines {
      // Parse @typedef {type} TypeName - Description
      if line.to_lowercase().contains("@typedef") {
        if let Some(typedef_pos) = line.to_lowercase().find("@typedef") {
          let after_typedef = &line[typedef_pos + 8..].trim(); // Skip "@typedef"
          
          // Handle @typedef {function} CallbackName or @typedef {Function} CallbackName
          if let Some(start_brace) = after_typedef.find('{') {
            if let Some(end_brace) = after_typedef.find('}') {
              let type_def = &after_typedef[start_brace + 1..end_brace].trim().to_lowercase();
              let after_brace = &after_typedef[end_brace + 1..].trim();
              
              if type_def == "function" {
                // This is a callback typedef
                is_callback = true;
                let mut parts = after_brace.split_whitespace();
                let first = parts.next().unwrap_or("");
                let has_extra = parts.next().is_some();
                if !first.is_empty() && !has_extra {
                  typedef_name = Some(first.to_string());
                }
              } else {
                // Regular typedef
                let mut parts = after_brace.split_whitespace();
                let first = parts.next().unwrap_or("");
                let has_extra = parts.next().is_some();
                if !first.is_empty() && !has_extra {
                  typedef_name = Some(first.to_string());
                }
              }
            }
          } else {
            // Handle @typedef TypeName (without type specification)
            let mut parts = after_typedef.split_whitespace();
            let first = parts.next().unwrap_or("");
            let has_extra = parts.next().is_some();
            if !first.is_empty() && !has_extra {
              typedef_name = Some(first.to_string());
            }
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

    // Register the typedef definition if we found a name
    if let Some(name) = typedef_name {
      // Ignore invalid names (empty or containing whitespace)
      if name.is_empty() || name.split_whitespace().count() != 1 {
        return;
      }

      let throws_annotation = if !aggregated_error_types.is_empty() {
        Some(ThrowsAnnotation { error_types: aggregated_error_types.into_iter().collect(), is_documented: true })
      } else { None };

      let typedef_def = TypedefDefinition {
        name: name.clone(),
        throws_annotation,
        is_callback,
        span,
      };
      
      #[cfg(debug_assertions)]
      eprintln!("🔍 Found @typedef definition: {} (callback: {}) with throws: {:?}", 
                name, is_callback, typedef_def.throws_annotation);
      
      self.typedefs.insert(name, typedef_def);
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

  /// Analyze all comments in the module to find @typedef definitions
  pub fn analyze_module(&mut self, module: &Module) {
    // Visit the module to trigger comment analysis
    self.visit_module(module);
    
    // Also scan all comments directly to catch top-level @typedef definitions
    // that might not be attached to specific AST nodes
    self.scan_all_comments();
  }

  /// Scan all comments in the source for @typedef definitions
  fn scan_all_comments(&mut self) {
    // Get all leading comments throughout the file
    // This is a simplified approach - in a real implementation, you'd want to
    // iterate through all comment positions in the source map
    
    // For now, we rely on the Visit implementation to catch most cases
    // This method can be enhanced to scan comments more comprehensively
  }

  /// Get a typedef definition by name
  pub fn get_typedef(&self, name: &str) -> Option<&TypedefDefinition> {
    self.typedefs.get(name)
  }

  /// Get all typedef definitions
  pub fn get_all_typedefs(&self) -> &HashMap<String, TypedefDefinition> {
    &self.typedefs
  }

  /// Get callback-specific typedefs
  pub fn get_callback_typedefs(&self) -> HashMap<String, &TypedefDefinition> {
    self.typedefs.iter()
      .filter(|(_, typedef_def)| typedef_def.is_callback)
      .map(|(name, typedef_def)| (name.clone(), typedef_def))
      .collect()
  }

  /// Get non-callback typedefs
  pub fn get_regular_typedefs(&self) -> HashMap<String, &TypedefDefinition> {
    self.typedefs.iter()
      .filter(|(_, typedef_def)| !typedef_def.is_callback)
      .map(|(name, typedef_def)| (name.clone(), typedef_def))
      .collect()
  }
}

impl Visit for TypedefFinder {
  fn visit_module(&mut self, module: &Module) {
    // Check for leading comments on the module itself
    if let Some(comments) = self.comments.get_leading(module.span.lo()) {
      for comment in comments {
        self.parse_typedef_comment(&comment.text, comment.span);
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
        self.parse_typedef_comment(&comment.text, comment.span);
      }
    }
    
    stmt.visit_children_with(self);
  }

  fn visit_decl(&mut self, decl: &swc_ecma_ast::Decl) {
    // Check for leading comments on declarations
    if let Some(comments) = self.comments.get_leading(decl.span().lo()) {
      for comment in comments {
        self.parse_typedef_comment(&comment.text, comment.span);
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
        eprintln!("Failed to parse module in typedef_finder: {:?}", e);
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
  fn test_basic_function_typedef() {
    let code = r#"
      /**
       * @typedef {function} AsyncProcessor
       * @throws {ProcessingError} When processing fails
       * @throws {ValidationError} When data is invalid
       */
      
      function processAsync() {
        // function body
      }
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = TypedefFinder::new(comments);
    finder.analyze_module(&module);
    
    assert_eq!(finder.typedefs.len(), 1);
    
    let typedef_def = finder.get_typedef("AsyncProcessor").unwrap();
    assert_eq!(typedef_def.name, "AsyncProcessor");
    assert!(typedef_def.is_callback);
    
    if let Some(throws) = &typedef_def.throws_annotation {
      assert!(throws.is_documented);
      assert_eq!(throws.error_types.len(), 2);
      assert!(throws.error_types.contains(&"ProcessingError".to_string()));
      assert!(throws.error_types.contains(&"ValidationError".to_string()));
    } else {
      panic!("Expected throws annotation");
    }
  }

  #[test]
  fn test_object_typedef() {
    let code = r#"
      /**
       * @typedef {Object} UserData
       * @property {string} name - User's name
       * @property {string} email - User's email
       * @property {number} age - User's age
       */
      
      function processUser() {}
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = TypedefFinder::new(comments);
    finder.analyze_module(&module);
    
    assert_eq!(finder.typedefs.len(), 1);
    
    let typedef_def = finder.get_typedef("UserData").unwrap();
    assert_eq!(typedef_def.name, "UserData");
    assert!(!typedef_def.is_callback); // Object typedef, not a callback
    assert!(typedef_def.throws_annotation.is_none());
  }

  #[test]
  fn test_mixed_typedefs() {
    let code = r#"
      /**
       * @typedef {function} ErrorHandler
       * @throws {NetworkError} When network fails
       */
      
      /**
       * @typedef {Object} Config
       * @property {string} apiUrl - API endpoint URL
       * @property {number} timeout - Request timeout
       */
      
      /**
       * @typedef {function} SuccessCallback
       * @throws {ValidationError} When validation fails
       */
      
      function api() {}
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = TypedefFinder::new(comments);
    finder.analyze_module(&module);
    
    assert_eq!(finder.typedefs.len(), 3);
    
    // Test callback typedefs
    let callback_typedefs = finder.get_callback_typedefs();
    assert_eq!(callback_typedefs.len(), 2);
    assert!(callback_typedefs.contains_key("ErrorHandler"));
    assert!(callback_typedefs.contains_key("SuccessCallback"));
    
    // Test regular typedefs
    let regular_typedefs = finder.get_regular_typedefs();
    assert_eq!(regular_typedefs.len(), 1);
    assert!(regular_typedefs.contains_key("Config"));
    
    // Verify specific typedef
    let error_handler = finder.get_typedef("ErrorHandler").unwrap();
    assert!(error_handler.is_callback);
    if let Some(throws) = &error_handler.throws_annotation {
      assert!(throws.error_types.contains(&"NetworkError".to_string()));
    }
    
    let config = finder.get_typedef("Config").unwrap();
    assert!(!config.is_callback);
    assert!(config.throws_annotation.is_none());
  }

  #[test]
  fn test_typedef_without_type_specification() {
    let code = r#"
      /**
       * @typedef MyCustomType
       * @throws {CustomError} When custom operation fails
       */
      
      function process() {}
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = TypedefFinder::new(comments);
    finder.analyze_module(&module);
    
    assert_eq!(finder.typedefs.len(), 1);
    
    let typedef_def = finder.get_typedef("MyCustomType").unwrap();
    assert_eq!(typedef_def.name, "MyCustomType");
    assert!(!typedef_def.is_callback); // No {function} specified
    
    if let Some(throws) = &typedef_def.throws_annotation {
      assert!(throws.error_types.contains(&"CustomError".to_string()));
    }
  }

  #[test]
  fn test_complex_typedef_with_multiple_throws() {
    let code = r#"
      /**
       * Advanced data processor with comprehensive error handling
       * @typedef {function} DataProcessor
       * @param {Object} data - Input data to process
       * @param {Object} options - Processing options
       * @param {boolean} options.validate - Whether to validate input
       * @param {number} options.timeout - Processing timeout in ms
       * @returns {Promise<ProcessedData>} Processed data
       * @throws {ValidationError} When input data is invalid
       * @throws {TimeoutError} When processing exceeds timeout
       * @throws {NetworkError} When network request fails
       * @throws {DatabaseError} When database operation fails
       * @throws {AuthenticationError} When user is not authenticated
       * @example
       * // Usage example
       * const processor = (data, options) => {
       *   // implementation
       * };
       */
      
      function setupProcessor() {}
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = TypedefFinder::new(comments);
    finder.analyze_module(&module);
    
    assert_eq!(finder.typedefs.len(), 1);
    
    let typedef_def = finder.get_typedef("DataProcessor").unwrap();
    assert_eq!(typedef_def.name, "DataProcessor");
    assert!(typedef_def.is_callback);
    
    if let Some(throws) = &typedef_def.throws_annotation {
      assert!(throws.is_documented);
      assert_eq!(throws.error_types.len(), 5);
      assert!(throws.error_types.contains(&"ValidationError".to_string()));
      assert!(throws.error_types.contains(&"TimeoutError".to_string()));
      assert!(throws.error_types.contains(&"NetworkError".to_string()));
      assert!(throws.error_types.contains(&"DatabaseError".to_string()));
      assert!(throws.error_types.contains(&"AuthenticationError".to_string()));
    } else {
      panic!("Expected throws annotation");
    }
  }

  #[test]
  fn test_throws_without_braces() {
    let code = r#"
      /**
       * @typedef {function} LegacyProcessor
       * @throws Error, TypeError, CustomError when something goes wrong
       */
      
      function legacy() {}
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = TypedefFinder::new(comments);
    finder.analyze_module(&module);
    
    assert_eq!(finder.typedefs.len(), 1);
    
    let typedef_def = finder.get_typedef("LegacyProcessor").unwrap();
    if let Some(throws) = &typedef_def.throws_annotation {
      assert_eq!(throws.error_types.len(), 3);
      assert!(throws.error_types.contains(&"Error".to_string()));
      assert!(throws.error_types.contains(&"TypeError".to_string()));
      assert!(throws.error_types.contains(&"CustomError".to_string()));
    }
  }

  #[test]
  fn test_malformed_typedef_definitions() {
    let code = r#"
      /**
       * @typedef {function incomplete brace TypeName
       * @throws {ValidError} This should work
       */
      
      /**
       * @typedef {function}
       * @throws {AnotherError} Missing typedef name
       */
      
      /**
       * @typedef {function} ValidTypedef
       * @throws {TypeError incomplete brace
       * @throws missing type
       * @throws {} empty braces
       * @throws {ValidType} This should work
       */
      
      function test() {}
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = TypedefFinder::new(comments);
    finder.analyze_module(&module);
    
    // Should only find ValidTypedef (the one with a proper name)
    assert_eq!(finder.typedefs.len(), 1);
    
    let typedef_def = finder.get_typedef("ValidTypedef").unwrap();
    if let Some(throws) = &typedef_def.throws_annotation {
      // Should only parse the valid @throws
      assert_eq!(throws.error_types.len(), 1);
      assert!(throws.error_types.contains(&"ValidType".to_string()));
    }
  }

  #[test]
  fn test_case_insensitive_function_detection() {
    let code = r#"
      /**
       * @typedef {Function} UpperCaseProcessor
       * @throws {ProcessingError} When processing fails
       */
      
      /**
       * @typedef {FUNCTION} AllCapsProcessor
       * @throws {ValidationError} When validation fails
       */
      
      function test() {}
    "#;
    
    let (module, comments) = parse_code_with_comments(code);
    let mut finder = TypedefFinder::new(comments);
    finder.analyze_module(&module);
    
    assert_eq!(finder.typedefs.len(), 2);
    
    let upper_case = finder.get_typedef("UpperCaseProcessor").unwrap();
    assert!(upper_case.is_callback);
    
    let all_caps = finder.get_typedef("AllCapsProcessor").unwrap();
    assert!(all_caps.is_callback);
    
    // Both should be recognized as callbacks
    let callback_typedefs = finder.get_callback_typedefs();
    assert_eq!(callback_typedefs.len(), 2);
  }
}
