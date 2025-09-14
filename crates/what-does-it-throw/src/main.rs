extern crate what_does_it_throw;
extern crate swc_common;
extern crate swc_ecma_ast;
extern crate swc_ecma_parser;
extern crate swc_ecma_visit;
use std::{fs, env};

use self::swc_common::{sync::Lrc, SourceMap};
use what_does_it_throw::{analyze_code, UserSettings};

pub fn main() {
  let args: Vec<String> = env::args().collect();
  
  // Check if a file path was provided as argument
  if args.len() > 1 {
    let file_path = &args[1];
    let include_try_statements = args.iter().any(|arg| arg == "--include-try-statements");
    
    println!("=== Analyzing File: {} ===\n", file_path);
    analyze_specific_file(file_path, include_try_statements);
  } else {
    println!("=== JSDoc @throws Analysis Demo ===\n");
    demo_jsdoc_throws_analysis();
    
    println!("\n=== Original Sample Analysis ===\n");
    demo_original_sample_analysis();
  }
}

fn analyze_specific_file(file_path: &str, include_try_statements: bool) {
  let code = fs::read_to_string(file_path)
    .unwrap_or_else(|_| panic!("Could not read file: {}", file_path));
  
  let cm: Lrc<SourceMap> = Default::default();
  let user_settings = UserSettings {
    include_try_statement_throws: include_try_statements,
    ignore_statements: vec![], // No ignore statements for file analysis
  };
  
  let (result, _cm, _comments) = analyze_code(&code, cm, &user_settings);
  
  println!("📊 Analysis Summary:");
  println!("  - Functions with throws: {}", result.functions_with_throws.len());
  println!("  - Functions calls to throws: {}", result.calls_to_throws.len());
  println!();

  // Analyze functions by documentation status
  let mut documented_functions = Vec::new();
  let mut undocumented_functions = Vec::new();
  let mut partially_documented_functions = Vec::new();

  for function in &result.functions_with_throws {
    let has_annotation = function.throws_annotation.is_some();
    let documented_types: Vec<String> = function.throws_annotation
      .as_ref()
      .map(|ann| ann.error_types.clone())
      .unwrap_or_default();
    
    let actual_error_types: Vec<String> = function.throw_details
      .iter()
      .filter_map(|detail| detail.error_type.clone())
      .collect();

    if has_annotation {
      // Check if all actual throws are documented
      let all_documented = actual_error_types.iter()
        .all(|actual_type| {
          // Handle "variable: name" format
          if actual_type.starts_with("variable: ") {
            return false; // Variables are never documented by type
          }
          
          // Handle union types by checking if all individual types are documented
          if actual_type.starts_with("union: ") {
            let union_content = &actual_type[7..]; // Remove "union: " prefix
            let individual_types: Vec<&str> = union_content
              .split(" | ")
              .map(|t| t.trim())
              .collect();
            
            // Check if all individual types in the union are documented
            return individual_types.iter().all(|individual_type| {
              documented_types.contains(&individual_type.to_string())
            });
          }
          
          // Regular type checking
          documented_types.contains(actual_type)
        });
      
      if all_documented {
        documented_functions.push(function);
      } else {
        partially_documented_functions.push(function);
      }
    } else {
      undocumented_functions.push(function);
    }
  }

  println!("📝 Documentation Status:");
  println!("  ✅ Fully documented: {}", documented_functions.len());
  println!("  ⚠️  Partially documented: {}", partially_documented_functions.len());
  println!("  ❌ Undocumented: {}", undocumented_functions.len());
  
  if !documented_functions.is_empty() {
    println!("\n✅ FULLY DOCUMENTED FUNCTIONS:");
    for function in documented_functions.iter().take(3) {
      let pos = _cm.lookup_char_pos(function.throw_statement.lo());
      println!("  📍 {} ({}:{})", 
        function.function_or_method_name,
        pos.line,
        pos.col_display
      );
      
      if let Some(annotation) = &function.throws_annotation {
        println!("     📚 Documents: {}", annotation.error_types.join(", "));
      }
      
      let error_types: Vec<String> = function.throw_details
        .iter()
        .filter_map(|d| d.error_type.clone())
        .collect();
      if !error_types.is_empty() {
        println!("     🎯 Actually throws: {}", error_types.join(", "));
      }
      println!();
    }
  }

  if !partially_documented_functions.is_empty() {
    println!("⚠️  PARTIALLY DOCUMENTED FUNCTIONS:");
    for function in partially_documented_functions.iter().take(3) {
      let pos = _cm.lookup_char_pos(function.throw_statement.lo());
      println!("  📍 {} ({}:{})", 
        function.function_or_method_name,
        pos.line,
        pos.col_display
      );
      
      if let Some(annotation) = &function.throws_annotation {
        println!("     📚 Documents: {}", annotation.error_types.join(", "));
      }
      
      let error_types: Vec<String> = function.throw_details
        .iter()
        .filter_map(|d| d.error_type.clone())
        .collect();
      if !error_types.is_empty() {
        println!("     🎯 Actually throws: {}", error_types.join(", "));
        
        let empty_vec = vec![];
        let documented_types = function.throws_annotation
          .as_ref()
          .map(|ann| &ann.error_types)
          .unwrap_or(&empty_vec);
        
        let missing: Vec<String> = error_types.iter()
          .filter_map(|error_type| {
            // Skip variable types
            if error_type.starts_with("variable: ") {
              return None;
            }
            
            // Handle union types
            if error_type.starts_with("union: ") {
              let union_content = &error_type[7..]; // Remove "union: " prefix
              let individual_types: Vec<&str> = union_content
                .split(" | ")
                .map(|t| t.trim())
                .collect();
              
              // Find individual types that are not documented
              let undocumented_individuals: Vec<String> = individual_types
                .iter()
                .filter(|individual_type| !documented_types.contains(&individual_type.to_string()))
                .map(|t| t.to_string())
                .collect();
              
              // If all individual types are documented, don't report the union as missing
              if undocumented_individuals.is_empty() {
                return None;
              } else {
                // Return the individual missing types instead of the union
                return Some(undocumented_individuals.join(", "));
              }
            }
            
            // Regular type checking
            if !documented_types.contains(error_type) {
              Some(error_type.clone())
            } else {
              None
            }
          })
          .collect();
        
        if !missing.is_empty() {
          println!("     ❗ Missing documentation for: {}", missing.join(", "));
        }
      }
      println!();
    }
  }

  if !undocumented_functions.is_empty() {
    println!("❌ UNDOCUMENTED FUNCTIONS:");
    for function in undocumented_functions.iter().take(5) {
      let pos = _cm.lookup_char_pos(function.throw_statement.lo());
      println!("  📍 {} ({}:{})", 
        function.function_or_method_name,
        pos.line,
        pos.col_display
      );
      
      let error_types: Vec<String> = function.throw_details
        .iter()
        .filter_map(|d| d.error_type.clone())
        .collect();
      if !error_types.is_empty() {
        println!("     🎯 Throws: {}", error_types.join(", "));
      }
      
      // Show error messages for context
      let messages: Vec<String> = function.throw_details
        .iter()
        .filter_map(|d| d.error_message.clone())
        .collect();
      if !messages.is_empty() {
        println!("     💬 Messages: {}", messages.join("; "));
      }
      println!();
    }
  }

  // Error type analysis
  println!("🔍 ERROR TYPE ANALYSIS:");
  let mut built_in_errors = 0;
  let mut custom_errors = 0;
  let mut string_throws = 0;
  let mut variable_throws = 0;

  for function in &result.functions_with_throws {
    for detail in &function.throw_details {
      if let Some(error_type) = &detail.error_type {
        if error_type.starts_with("variable: ") {
          variable_throws += 1;
        } else if error_type.starts_with("union: ") {
          variable_throws += 1; // Count union types as smart variable analysis
        } else if detail.is_custom_error {
          custom_errors += 1;
        } else {
          built_in_errors += 1;
        }
      } else {
        string_throws += 1;
      }
    }
  }

  println!("  🏗️  Built-in errors (Error, TypeError, etc.): {}", built_in_errors);
  println!("  🎨 Custom error classes: {}", custom_errors);
  println!("  📝 String literals: {}", string_throws);
  println!("  🔗 Variable references: {}", variable_throws);
  println!();

  // Call chain analysis
  println!("🔄 CALL CHAIN ANALYSIS:");
  println!("  Functions that call throwing functions: {}", result.calls_to_throws.len());
  if !result.calls_to_throws.is_empty() {
    println!("  Examples:");
    for call in result.calls_to_throws.iter().take(3) {
      let pos = _cm.lookup_char_pos(call.call_span.lo());
      println!("    📞 {} calls {} ({}:{})", 
        call.call_function_or_method_name,
        call.throw_map.function_or_method_name,
        pos.line,
        pos.col_display
      );
    }
  }
}

fn demo_jsdoc_throws_analysis() {
  let jsdoc_code = fs::read_to_string("src/fixtures/jsdocThrowsSuppression.js")
    .expect("Could not read jsdocThrowsSuppression.js fixture");
  
  let cm: Lrc<SourceMap> = Default::default();
  let user_settings = UserSettings {
    include_try_statement_throws: true,
    ignore_statements: vec![], // No ignore statements for this demo
  };
  
  let (result, _cm, _comments) = analyze_code(&jsdoc_code, cm, &user_settings);
  
  println!("📊 Analysis Summary:");
  println!("  - Functions with throws: {}", result.functions_with_throws.len());
  println!("  - Functions calls to throws: {}", result.calls_to_throws.len());
  println!();

  // Analyze functions by documentation status
  let mut documented_functions = Vec::new();
  let mut undocumented_functions = Vec::new();
  let mut partially_documented_functions = Vec::new();

  for function in &result.functions_with_throws {
    let has_annotation = function.throws_annotation.is_some();
    let documented_types: Vec<String> = function.throws_annotation
      .as_ref()
      .map(|ann| ann.error_types.clone())
      .unwrap_or_default();
    
    let actual_error_types: Vec<String> = function.throw_details
      .iter()
      .filter_map(|detail| detail.error_type.clone())
      .collect();

    if has_annotation {
      // Check if all actual throws are documented
      let all_documented = actual_error_types.iter()
        .all(|actual_type| {
          // Handle "variable: name" format
          if actual_type.starts_with("variable: ") {
            return false; // Variables are never documented by type
          }
          
          // Handle union types by checking if all individual types are documented
          if actual_type.starts_with("union: ") {
            let union_content = &actual_type[7..]; // Remove "union: " prefix
            let individual_types: Vec<&str> = union_content
              .split(" | ")
              .map(|t| t.trim())
              .collect();
            
            // Check if all individual types in the union are documented
            return individual_types.iter().all(|individual_type| {
              documented_types.contains(&individual_type.to_string())
            });
          }
          
          // Regular type checking
          documented_types.contains(actual_type)
        });
      
      if all_documented {
        documented_functions.push(function);
      } else {
        partially_documented_functions.push(function);
      }
    } else {
      undocumented_functions.push(function);
    }
  }

  println!("📝 Documentation Status:");
  println!("  ✅ Fully documented: {}", documented_functions.len());
  println!("  ⚠️  Partially documented: {}", partially_documented_functions.len());
  println!("  ❌ Undocumented: {}", undocumented_functions.len());
  println!();

  // Show examples of each category
  if !documented_functions.is_empty() {
    println!("✅ FULLY DOCUMENTED FUNCTIONS:");
    for function in documented_functions.iter().take(3) {
      let annotation = function.throws_annotation.as_ref().unwrap();
      println!("  📍 {} ({}:{})", 
        function.function_or_method_name,
        _cm.lookup_char_pos(function.throw_statement.lo()).line,
        _cm.lookup_char_pos(function.throw_statement.lo()).col_display
      );
      println!("     📚 Documents: {}", annotation.error_types.join(", "));
      
      let actual_types: Vec<String> = function.throw_details
        .iter()
        .filter_map(|d| d.error_type.clone())
        .collect();
      println!("     🎯 Actually throws: {}", actual_types.join(", "));
      println!();
    }
  }

  if !partially_documented_functions.is_empty() {
    println!("⚠️  PARTIALLY DOCUMENTED FUNCTIONS:");
    for function in partially_documented_functions.iter().take(3) {
      let annotation = function.throws_annotation.as_ref().unwrap();
      println!("  📍 {} ({}:{})", 
        function.function_or_method_name,
        _cm.lookup_char_pos(function.throw_statement.lo()).line,
        _cm.lookup_char_pos(function.throw_statement.lo()).col_display
      );
      println!("     📚 Documents: {}", annotation.error_types.join(", "));
      
      let actual_types: Vec<String> = function.throw_details
        .iter()
        .filter_map(|d| d.error_type.clone())
        .collect();
      println!("     🎯 Actually throws: {}", actual_types.join(", "));
      
      // Show what's missing
      let undocumented: Vec<String> = actual_types.iter()
        .filter(|actual| {
          if actual.starts_with("variable: ") { return false; }
          !annotation.error_types.contains(actual)
        })
        .cloned()
        .collect();
      if !undocumented.is_empty() {
        println!("     ❗ Missing documentation for: {}", undocumented.join(", "));
      }
      println!();
    }
  }

  if !undocumented_functions.is_empty() {
    println!("❌ UNDOCUMENTED FUNCTIONS:");
    for function in undocumented_functions.iter().take(5) {
      println!("  📍 {} ({}:{})", 
        function.function_or_method_name,
        _cm.lookup_char_pos(function.throw_statement.lo()).line,
        _cm.lookup_char_pos(function.throw_statement.lo()).col_display
      );
      
      let error_types: Vec<String> = function.throw_details
        .iter()
        .filter_map(|d| d.error_type.clone())
        .collect();
      if !error_types.is_empty() {
        println!("     🎯 Throws: {}", error_types.join(", "));
      }
      
      // Show error messages for context
      let messages: Vec<String> = function.throw_details
        .iter()
        .filter_map(|d| d.error_message.clone())
        .collect();
      if !messages.is_empty() {
        println!("     💬 Messages: {}", messages.join("; "));
      }
      println!();
    }
  }

  // Error type analysis
  println!("🔍 ERROR TYPE ANALYSIS:");
  let mut built_in_errors = 0;
  let mut custom_errors = 0;
  let mut string_throws = 0;
  let mut variable_throws = 0;

  for function in &result.functions_with_throws {
    for detail in &function.throw_details {
      if let Some(error_type) = &detail.error_type {
        if error_type.starts_with("variable: ") {
          variable_throws += 1;
        } else if detail.is_custom_error {
          custom_errors += 1;
        } else {
          built_in_errors += 1;
        }
      } else {
        string_throws += 1;
      }
    }
  }

  println!("  🏗️  Built-in errors (Error, TypeError, etc.): {}", built_in_errors);
  println!("  🎨 Custom error classes: {}", custom_errors);
  println!("  📝 String literals: {}", string_throws);
  println!("  🔗 Variable references: {}", variable_throws);
  println!();

  // Call chain analysis
  println!("🔄 CALL CHAIN ANALYSIS:");
  println!("  Functions that call throwing functions: {}", result.calls_to_throws.len());
  if !result.calls_to_throws.is_empty() {
    println!("  Examples:");
    for call in result.calls_to_throws.iter().take(3) {
      println!("    📞 {} calls {} ({}:{})", 
        call.call_function_or_method_name,
        call.throw_map.function_or_method_name,
        _cm.lookup_char_pos(call.call_span.lo()).line,
        _cm.lookup_char_pos(call.call_span.lo()).col_display
      );
    }
  }
}

fn demo_original_sample_analysis() {
  let sample_code = fs::read_to_string("src/fixtures/sample.ts")
    .expect("Something went wrong reading the file");
  let cm: Lrc<SourceMap> = Default::default();
  let user_settings = UserSettings {
    include_try_statement_throws: false,
    ignore_statements: vec!["@it-throws".to_string()],
  };
  let (result, _cm, _comments) = analyze_code(&sample_code, cm, &user_settings);
  for import in result.import_sources.into_iter() {
    println!("Imported {}", import);
  }
  for fun in result.functions_with_throws.clone().into_iter() {
    let start = _cm.lookup_char_pos(fun.throw_statement.lo());
    let end = _cm.lookup_char_pos(fun.throw_statement.hi());
    println!(
      "Function throws: {}, className {}",
      fun.function_or_method_name,
      fun.class_name.unwrap_or_else(|| "NOT_SET".to_string())
    );
    println!(
      "From line {} column {} to line {} column {}",
      start.line, start.col_display, end.line, end.col_display
    );
    for span in &fun.throw_spans {
      let start = _cm.lookup_char_pos(span.lo());
      let end = _cm.lookup_char_pos(span.hi());
      println!(
        "  Throw from line {} column {} to line {} column {}",
        start.line, start.col_display, end.line, end.col_display
      );
    }
  }

  for throw_id in result.functions_with_throws.into_iter() {
    println!("throw id: {}", throw_id.id);
  }

  println!("------- Calls to throws --------");
  for call in result.calls_to_throws.into_iter() {
    let start = _cm.lookup_char_pos(call.call_span.lo());
    let end = _cm.lookup_char_pos(call.call_span.hi());
    println!("Call throws: {}", call.id);
    println!(
      "From line {} column {} to line {} column {}",
      start.line, start.col_display, end.line, end.col_display
    );
  }

  println!("-------- Imported identifiers usages --------");
  for identifier_usage in result.imported_identifier_usages.into_iter() {
    let start = _cm.lookup_char_pos(identifier_usage.usage_span.lo());
    let end = _cm.lookup_char_pos(identifier_usage.usage_span.hi());
    let identifier_name = &identifier_usage.id;
    println!(
      "{} From line {} column {} to line {} column {}",
      identifier_name, start.line, start.col_display, end.line, end.col_display
    );
  }
}
