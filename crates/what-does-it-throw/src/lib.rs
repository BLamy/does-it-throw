pub mod call_finder;
pub mod import_usage_finder;
pub mod throw_finder;
pub mod function_finder;
pub mod try_catch_finder;
pub mod callback_finder;
pub mod typedef_finder;
pub mod param_finder;
use call_finder::{CallFinder, CallToThrowMap};
use import_usage_finder::ImportUsageFinder;
use function_finder::{FunctionFinder, FunctionMap};
use callback_finder::CallbackFinder;
use typedef_finder::TypedefFinder;
use param_finder::ParamFinder;
use swc_common::comments::{SingleThreadedComments, Comments};
use swc_common::Spanned;
use throw_finder::{IdentifierUsage, ThrowAnalyzer, ThrowMap, ThrowFinderSettings, TypeRegistry};
use try_catch_finder::{TryCatchFinder, CatchAnalysis};
extern crate swc_common;
extern crate swc_ecma_ast;
extern crate swc_ecma_parser;
extern crate swc_ecma_visit;

use std::collections::{HashMap, HashSet};

use std::vec;

use self::swc_common::{sync::Lrc, SourceMap};
use self::swc_ecma_ast::EsVersion;
use self::swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax};
use self::swc_ecma_visit::{Visit, VisitWith};
use swc_common::Span;

/// Visitor to analyze function calls and direct throws within a specific try block
struct TryBlockCallAnalyzer {
  function_calls: Vec<String>,
  direct_throws: Vec<String>, // Track direct throw statements
}

impl TryBlockCallAnalyzer {
  fn new() -> Self {
    Self {
      function_calls: Vec::new(),
      direct_throws: Vec::new(),
    }
  }
}

impl Visit for TryBlockCallAnalyzer {
  fn visit_call_expr(&mut self, call_expr: &swc_ecma_ast::CallExpr) {
    // Extract function name from call expression
    if let swc_ecma_ast::Callee::Expr(expr) = &call_expr.callee {
      if let swc_ecma_ast::Expr::Ident(ident) = &**expr {
        self.function_calls.push(ident.sym.to_string());
      }
    }
    // Continue visiting child nodes
    call_expr.visit_children_with(self);
  }
  
  fn visit_throw_stmt(&mut self, throw_stmt: &swc_ecma_ast::ThrowStmt) {
    // Extract error type from direct throw statement
    match &*throw_stmt.arg {
      swc_ecma_ast::Expr::New(new_expr) => {
        if let swc_ecma_ast::Expr::Ident(ident) = &*new_expr.callee {
          let error_type = ident.sym.to_string();
          if !self.direct_throws.contains(&error_type) {
            self.direct_throws.push(error_type);
          }
        }
      }
      _ => {
        // For other types of throws (strings, variables), add a generic "Error" type
        if !self.direct_throws.contains(&"Error".to_string()) {
          self.direct_throws.push("Error".to_string());
        }
      }
    }
    // Continue visiting child nodes
    throw_stmt.visit_children_with(self);
  }
  
  // Don't visit nested try blocks - we only want calls directly in this try block
  fn visit_try_stmt(&mut self, _try_stmt: &swc_ecma_ast::TryStmt) {
    // Don't visit children to avoid analyzing nested try blocks
  }
}

/// Populates catch analyses with actual thrown errors by analyzing function calls within try blocks
/// and looking up their actual error types from the throw analysis data
fn populate_catch_analyses_with_throws(
  mut catch_analyses: Vec<CatchAnalysis>,
  functions_with_throws: &HashSet<ThrowMap>,
  module: &swc_ecma_ast::Module,
) -> Vec<CatchAnalysis> {
  
  println!("🔧 Analyzing {} catch blocks for error flow using real call graph data", catch_analyses.len());
  
  // Create a lookup map from function names to their thrown error types
  let mut function_error_map: HashMap<String, Vec<String>> = HashMap::new();
  
  for throw_map in functions_with_throws {
    let error_types: Vec<String> = throw_map.throw_details
      .iter()
      .filter_map(|detail| detail.error_type.clone())
      .collect();
    
    if !error_types.is_empty() {
      function_error_map.insert(throw_map.function_or_method_name.clone(), error_types);
    }
  }
  
  println!("  📋 Built function error map with {} throwing functions", function_error_map.len());
  for (func_name, errors) in &function_error_map {
    println!("    - {}: {:?}", func_name, errors);
  }
  
  // Find all try statements in the module and match them with catch analyses
  let mut try_finder = TryStatementFinder::new();
  try_finder.visit_module(module);
  
  for catch_analysis in &mut catch_analyses {
    let try_span = catch_analysis.try_span;
    let handled_errors = catch_analysis.errors_handled_in_catch.clone();
    println!("  🎯 Analyzing try block span: {:?}", try_span);
    
    // Find the corresponding try statement for this catch analysis
    if let Some(try_block) = try_finder.find_try_block_by_span(&try_span) {
      // Analyze function calls within this specific try block
      let mut call_analyzer = TryBlockCallAnalyzer::new();
      call_analyzer.visit_block_stmt(try_block);
      
      println!("    🔍 Found {} function calls in try block: {:?}", 
        call_analyzer.function_calls.len(), 
        call_analyzer.function_calls
      );
      println!("    🔍 Found {} direct throws in try block: {:?}", 
        call_analyzer.direct_throws.len(), 
        call_analyzer.direct_throws
      );
      
      // Look up what errors these function calls can throw
      let mut thrown_errors = Vec::new();
      for function_call in &call_analyzer.function_calls {
        if let Some(errors) = function_error_map.get(function_call) {
          for error in errors {
            if !thrown_errors.contains(error) {
              thrown_errors.push(error.clone());
            }
          }
          println!("    ✅ {} can throw: {:?}", function_call, errors);
        } else {
          println!("    ⚠️  {} not found in error map (might not throw)", function_call);
        }
      }
      
      // Add direct throws from the try block
      for direct_throw in &call_analyzer.direct_throws {
        if !thrown_errors.contains(direct_throw) {
          thrown_errors.push(direct_throw.clone());
        }
        println!("    ✅ Direct throw found: {}", direct_throw);
      }
      
      // Add the actual thrown errors to the catch analysis
      for error_type in thrown_errors {
        catch_analysis.add_thrown_error(error_type);
      }
    } else {
      println!("    ❌ Could not find try block for this catch analysis");
      
      // Fallback: use the catch handlers as indicators of what errors are thrown
      for error_type in &handled_errors {
        if function_error_map.values().any(|errors| errors.contains(error_type)) {
          catch_analysis.add_thrown_error(error_type.clone());
          println!("    ✅ Fallback: Confirmed {} is actually thrown by some function", error_type);
        }
      }
    }

    // Recalculate error flow with the real data
    catch_analysis.calculate_error_flow();
    
    println!("    📊 After calculation:");
    println!("      - Thrown: {:?}", catch_analysis.errors_thrown_in_try);
    println!("      - Handled: {:?}", catch_analysis.errors_handled_in_catch);
    println!("      - Effectively caught: {:?}", catch_analysis.errors_effectively_caught);
    println!("      - Propagated: {:?}", catch_analysis.errors_propagated);
  }

  catch_analyses
}

/// Visitor to find all try statements in the module and map them by span
struct TryStatementFinder {
  try_blocks: Vec<(Span, swc_ecma_ast::BlockStmt)>,
}

impl TryStatementFinder {
  fn new() -> Self {
    Self {
      try_blocks: Vec::new(),
    }
  }
  
  fn find_try_block_by_span(&self, target_span: &Span) -> Option<&swc_ecma_ast::BlockStmt> {
    self.try_blocks
      .iter()
      .find(|(span, _)| span.lo() == target_span.lo() && span.hi() == target_span.hi())
      .map(|(_, block)| block)
  }
}

impl Visit for TryStatementFinder {
  fn visit_try_stmt(&mut self, try_stmt: &swc_ecma_ast::TryStmt) {
    self.try_blocks.push((try_stmt.block.span, try_stmt.block.clone()));
    // Continue visiting child nodes
    try_stmt.visit_children_with(self);
  }
}


#[derive(Default)]
pub struct AnalysisResult {
  pub functions_with_throws: HashSet<ThrowMap>,
  pub calls_to_throws: HashSet<CallToThrowMap>,
  pub json_parse_calls: Vec<String>,
  pub fs_access_calls: Vec<String>,
  pub import_sources: HashSet<String>,
  pub imported_identifiers: Vec<String>,
  pub imported_identifier_usages: HashSet<IdentifierUsage>,
  pub catch_analyses: Vec<CatchAnalysis>, // New: error flow analysis for try-catch blocks
  pub unused_it_throws_comments: Vec<Span>, // Track unused @it-throws comments
  pub all_functions: HashSet<FunctionMap>, // All functions (throwing and non-throwing) for JSDoc checking
  pub inline_callback_allowed_throws: std::collections::HashMap<Span, Vec<String>>, // Map inline callback span -> allowed throws
}


pub struct UserSettings {
  pub include_try_statement_throws: bool,
  pub ignore_statements: Vec<String>,
}



/// Simple propagation without catch analysis filtering - used when include_try_statement_throws is true
fn propagate_throws_to_callers_without_catch_filtering(
  functions_with_throws: HashSet<ThrowMap>,
  calls_to_throws: &HashSet<CallToThrowMap>,
  all_functions: &HashSet<FunctionMap>,
) -> HashSet<ThrowMap> {
  let mut result_functions = functions_with_throws;
  
  // Simple propagation from called functions to callers
  for call in calls_to_throws {
    // Find the function information for the caller
    if let Some(function_info) = all_functions.iter().find(|f| f.id == call.id) {
      // Check if this function already has throws
      let has_existing_throws = result_functions.iter().any(|f| f.id == function_info.id);
      
      if has_existing_throws {
        // Merge with existing throws
        if let Some(mut existing_throw_map) = result_functions.take(&ThrowMap {
          throw_spans: vec![function_info.span],
          throw_statement: function_info.span,
          function_or_method_name: function_info.name.clone(),
          class_name: function_info.class_name.clone(),
          id: function_info.id.clone(),
          throw_details: vec![], // dummy for lookup
          throws_annotation: None, // dummy for lookup
        }) {
          // Merge propagated throws with existing ones
          for propagated_throw in &call.throw_map.throw_details {
            if !existing_throw_map.throw_details.iter().any(|existing| {
              existing.error_type == propagated_throw.error_type
            }) {
              existing_throw_map.throw_details.push(propagated_throw.clone());
            }
          }
          result_functions.insert(existing_throw_map);
        }
      } else {
        // Create new throw map for propagated errors
        let new_throw_map = ThrowMap {
          throw_spans: vec![function_info.span],
          throw_statement: function_info.span,
          function_or_method_name: function_info.name.clone(),
          class_name: function_info.class_name.clone(),
          id: function_info.id.clone(),
          throw_details: call.throw_map.throw_details.clone(),
          throws_annotation: function_info.throws_annotation.clone(),
        };
        
        result_functions.insert(new_throw_map);
      }
    }
  }
  
  result_functions
}

/// Propagates throws from called functions to calling functions using complete function information
/// This creates ThrowMap entries for functions that don't directly throw but call functions that do
/// Now enhanced with catch analysis to filter out effectively caught errors
fn propagate_throws_to_callers(
  mut functions_with_throws: HashSet<ThrowMap>,
  calls_to_throws: &HashSet<CallToThrowMap>,
  all_functions: &HashSet<FunctionMap>,
  catch_analyses: &[CatchAnalysis],
) -> HashSet<ThrowMap> {
  
  // First, filter existing direct throws through catch analysis
  let mut filtered_functions_with_throws = HashSet::new();
  
  for throw_map in functions_with_throws.drain() {
    // Find the function info to get the function span
    if let Some(function_info) = all_functions.iter().find(|f| f.id == throw_map.id) {
      let effectively_caught_errors = get_effectively_caught_errors_for_function(
        function_info.span, 
        catch_analyses
      );
      
      println!("🔧 Filtering original function: {} ({})", function_info.name, function_info.id);
      println!("   📍 Function span: {:?}", function_info.span);
      println!("   🎯 Effectively caught errors: {:?}", effectively_caught_errors);
      println!("   📝 Original throw details: {:?}", throw_map.throw_details.iter().map(|d| &d.error_type).collect::<Vec<_>>());
      
      // Filter the throw details to exclude effectively caught errors
      let mut filtered_throw_details = throw_map.throw_details.clone();
      filtered_throw_details.retain(|throw_detail| {
        if let Some(ref error_type) = throw_detail.error_type {
          // Don't keep errors that are effectively caught
          let should_keep = !effectively_caught_errors.contains(error_type);
          println!("     🔍 Error type '{}': keep={}", error_type, should_keep);
          should_keep
        } else {
          println!("     🔍 String/other throw: keeping");
          true // Keep string throws and other types
        }
      });
      
      println!("   📝 Filtered throw details: {:?}", filtered_throw_details.iter().map(|d| &d.error_type).collect::<Vec<_>>());
      
      // Only keep the function if it has unhandled throws
      if !filtered_throw_details.is_empty() {
        let mut filtered_throw_map = throw_map.clone();
        filtered_throw_map.throw_details = filtered_throw_details;
        filtered_functions_with_throws.insert(filtered_throw_map);
        println!("   ✅ Kept function (has unhandled throws)");
      } else {
        println!("   ❌ Filtered out function (all throws effectively caught)");
      }
    } else {
      // If we can't find function info, we need to check if this function's throws
      // are all within try blocks by looking at the throw spans directly
      println!("🔧 No function info found for: {}, checking throws directly", throw_map.function_or_method_name);
      
      let mut has_unhandled_throws = false;
      for (i, throw_detail) in throw_map.throw_details.iter().enumerate() {
        if let Some(throw_span) = throw_map.throw_spans.get(i) {
          let is_within_try_block = catch_analyses.iter().any(|catch_analysis| {
            let span_within_try = throw_span.lo() >= catch_analysis.try_span.lo() 
              && throw_span.hi() <= catch_analysis.try_span.hi();
            let is_effectively_caught = if let Some(ref error_type) = throw_detail.error_type {
              catch_analysis.errors_effectively_caught.contains(error_type)
            } else {
              false // String throws are not effectively caught by instanceof checks
            };
            span_within_try && is_effectively_caught
          });
          
          if !is_within_try_block {
            has_unhandled_throws = true;
            println!("    🔍 Found unhandled throw at span {:?}", throw_span);
            break;
          } else {
            println!("    ❌ Throw at span {:?} is within try block and effectively caught", throw_span);
          }
        } else {
          // No span info, conservatively keep it
          has_unhandled_throws = true;
          break;
        }
      }
      
      if has_unhandled_throws {
        println!("    ✅ Keeping function (has unhandled throws)");
        filtered_functions_with_throws.insert(throw_map);
      } else {
        println!("    ❌ Filtering out function (all throws effectively caught)");
      }
    }
  }
  
  // Now handle propagation from called functions
  for call in calls_to_throws {
    // Find the function information for the caller
    if let Some(function_info) = all_functions.iter().find(|f| f.id == call.id) {
      // Check if this function already has (filtered) throws
      let has_existing_throws = filtered_functions_with_throws.iter().any(|f| f.id == function_info.id);
      
      // Get effectively caught errors for this function
      let effectively_caught_errors = get_effectively_caught_errors_for_function(
        function_info.span, 
        catch_analyses
      );
      
      // Filter the called function's throws to exclude effectively caught errors
      let mut propagated_throws = call.throw_map.throw_details.clone();
      propagated_throws.retain(|throw_detail| {
        if let Some(ref error_type) = throw_detail.error_type {
          // Don't propagate errors that are effectively caught
          !effectively_caught_errors.contains(error_type)
        } else {
          true // Keep string throws and other types
        }
      });
      
      if !propagated_throws.is_empty() {
        if has_existing_throws {
          // Merge with existing throws
          if let Some(mut existing_throw_map) = filtered_functions_with_throws.take(&ThrowMap {
            throw_spans: vec![function_info.span],
            throw_statement: function_info.span,
            function_or_method_name: function_info.name.clone(),
            class_name: function_info.class_name.clone(),
            id: function_info.id.clone(),
            throw_details: vec![], // dummy for lookup
            throws_annotation: None, // dummy for lookup
          }) {
            // Merge propagated throws with existing ones
            for propagated_throw in propagated_throws {
              if !existing_throw_map.throw_details.iter().any(|existing| {
                existing.error_type == propagated_throw.error_type
              }) {
                existing_throw_map.throw_details.push(propagated_throw);
              }
            }
            filtered_functions_with_throws.insert(existing_throw_map);
          }
        } else {
          // Create new throw map for propagated errors
          let new_throw_map = ThrowMap {
            throw_spans: vec![function_info.span],
            throw_statement: function_info.span,
            function_or_method_name: function_info.name.clone(),
            class_name: function_info.class_name.clone(),
            id: function_info.id.clone(),
            throw_details: propagated_throws,
            throws_annotation: function_info.throws_annotation.clone(),
          };
          
          println!("🚀 Propagated throws to caller: {} ({})", 
            function_info.name, 
            function_info.id
          );
          
          filtered_functions_with_throws.insert(new_throw_map);
        }
      }
    }
  }
  
  filtered_functions_with_throws
}

/// Get all error types that are effectively caught (handled with instanceof and not re-thrown)
/// within a specific function's try-catch blocks
fn get_effectively_caught_errors_for_function(
  function_span: swc_common::Span, 
  catch_analyses: &[CatchAnalysis]
) -> HashSet<String> {
  let mut effectively_caught = HashSet::new();
  
  for catch_analysis in catch_analyses {
    // Check if this catch block is within the function's span
    if catch_analysis.try_span.lo() >= function_span.lo() && catch_analysis.try_span.hi() <= function_span.hi() {
      // Add errors that are effectively caught (handled but not re-thrown)
      for error_type in &catch_analysis.errors_effectively_caught {
        effectively_caught.insert(error_type.clone());
      }
    }
  }
  
  effectively_caught
}

/// Filters functions to exclude throws that are within try blocks when include_try_statements is false.
/// This is applied AFTER catch analysis, so it only excludes throws that are within try blocks
/// but respects the sophisticated catch analysis results.
fn filter_functions_exclude_try_block_throws(
  functions_with_throws: HashSet<ThrowMap>,
  all_functions: &HashSet<FunctionMap>,
  catch_analyses: &[CatchAnalysis],
) -> HashSet<ThrowMap> {
  let mut filtered_functions = HashSet::new();
  
  println!("🔧 Filtering functions to exclude try block throws:");
  println!("  - Input: {} functions with throws", functions_with_throws.len());
  println!("  - Total catch analyses: {}", catch_analyses.len());

  for throw_map in functions_with_throws {
    println!("  🔍 Processing function: {} ({})", throw_map.function_or_method_name, throw_map.id);
    // Find the function info to get the function span
    if let Some(function_info) = all_functions.iter().find(|f| f.id == throw_map.id) {
      // Filter throw details to exclude those within try blocks (unless effectively caught was already handled)
      let mut filtered_throw_details = Vec::new();
      
      for (i, throw_detail) in throw_map.throw_details.iter().enumerate() {
        println!("    📝 Checking throw detail {}: {:?}", i, throw_detail.error_type);
        // Check if any of the throw spans are within try blocks
        let corresponding_span = throw_map.throw_spans.get(i);
        if let Some(throw_span) = corresponding_span {
          println!("      📍 Throw span: {:?}", throw_span);
          let is_within_try_block = catch_analyses.iter().any(|catch_analysis| {
            let span_within_try = throw_span.lo() >= catch_analysis.try_span.lo() 
              && throw_span.hi() <= catch_analysis.try_span.hi();
            let try_within_function = catch_analysis.try_span.lo() >= function_info.span.lo() 
              && catch_analysis.try_span.hi() <= function_info.span.hi();
            let result = span_within_try && try_within_function;
            if result {
              println!("      ✅ Found matching try block: try_span={:?} within function_span={:?}", 
                catch_analysis.try_span, function_info.span);
            }
            result
          });
          
          println!("      🔍 is_within_try_block: {}", is_within_try_block);
          
          if !is_within_try_block {
            // Keep throws that are not within try blocks
            filtered_throw_details.push(throw_detail.clone());
            println!("      ✅ Kept throw detail (not in try block)");
          } else {
            println!("      ❌ Filtered out throw detail (in try block)");
          }
        } else {
          // No corresponding span, keep the throw detail
          filtered_throw_details.push(throw_detail.clone());
          println!("      ✅ Kept throw detail (no span info)");
        }
      }
      
      // Only keep the function if it has remaining throws
      if !filtered_throw_details.is_empty() {
        let mut filtered_throw_map = throw_map.clone();
        filtered_throw_map.throw_details = filtered_throw_details;
        // Also filter throw spans to match
        filtered_throw_map.throw_spans.retain(|throw_span| {
          !catch_analyses.iter().any(|catch_analysis| {
            throw_span.lo() >= catch_analysis.try_span.lo() 
              && throw_span.hi() <= catch_analysis.try_span.hi()
              && catch_analysis.try_span.lo() >= function_info.span.lo() 
              && catch_analysis.try_span.hi() <= function_info.span.hi()
          })
        });
        filtered_functions.insert(filtered_throw_map);
      }
    } else {
      // If we can't find function info, keep the original throw map
      filtered_functions.insert(throw_map);
    }
  }
  
  filtered_functions
}

/// Filters calls_to_throws to exclude calls that are effectively caught by catch blocks
/// AND calls to functions that are no longer available (were filtered out due to effective catching).
/// This prevents blue squiggles on function calls that are inside try blocks where the 
/// thrown errors are effectively caught, and also prevents propagation from functions
/// that had all their throws effectively caught.
fn filter_calls_through_catch_analysis_and_function_availability(
  calls_to_throws: HashSet<CallToThrowMap>,
  all_functions: &HashSet<FunctionMap>,
  catch_analyses: &[CatchAnalysis],
  available_throwing_functions: &HashSet<String>,
) -> HashSet<CallToThrowMap> {
  let mut filtered_calls = HashSet::new();

  for call in calls_to_throws {
    // First check if the called function is still available (not filtered out)
    let called_function_still_throws = available_throwing_functions.contains(&call.throw_map.id);
    
    if !called_function_still_throws {
      println!("🔧 Filtering out call to {} because function was filtered out (all throws effectively caught)", 
        call.throw_map.function_or_method_name);
      continue;
    }
    
    // Then check the existing logic for effectively caught calls
    if let Some(_caller_function) = all_functions.iter().find(|f| f.id == call.id) {
      // Check if this call is within a try block and if the errors are effectively caught
      let call_is_effectively_caught = catch_analyses.iter().any(|catch_analysis| {
        // Check if the call span is within the try block span
        call.call_span.lo() >= catch_analysis.try_span.lo() 
          && call.call_span.hi() <= catch_analysis.try_span.hi()
          && {
            // Check if all errors thrown by the called function are effectively caught
            call.throw_map.throw_details.iter().all(|throw_detail| {
              if let Some(ref error_type) = throw_detail.error_type {
                catch_analysis.errors_effectively_caught.contains(error_type)
              } else {
                false // String throws are not effectively caught by instanceof checks
              }
            })
          }
      });
      
      if !call_is_effectively_caught {
        // Only keep calls that are not effectively caught
        filtered_calls.insert(call);
      } else {
        println!("🔧 Filtering out call to {} because it's effectively caught", 
          call.throw_map.function_or_method_name);
      }
    } else {
      // If we can't find function info, keep the original call
      filtered_calls.insert(call);
    }
  }
  
  filtered_calls
}

/// Find unused @it-throws comments by checking if they actually suppress any diagnostics
fn find_unused_it_throws_comments(
  comments: &Lrc<SingleThreadedComments>,
  module: &swc_ecma_ast::Module,
  ignore_statements: &[String],
  throw_analyzer: &ThrowAnalyzer,
  call_finder: &CallFinder,
) -> Vec<Span> {
  // Step 1: Collect all @it-throws comments by visiting all AST nodes and checking for comments
  use swc_ecma_visit::{Visit};
  
  struct CommentCollector<'a> {
    all_it_throws_comments: HashSet<Span>,
    ignore_statements: Vec<String>,
    comments: &'a Lrc<SingleThreadedComments>,
  }

  impl<'a> CommentCollector<'a> {
    fn new(ignore_statements: &[String], comments: &'a Lrc<SingleThreadedComments>) -> Self {
      Self {
        all_it_throws_comments: HashSet::new(),
        ignore_statements: ignore_statements.to_vec(),
        comments,
      }
    }
    
    fn check_comments_at_position(&mut self, pos: swc_common::BytePos) {
      // Check leading comments
      if let Some(leading_comments) = self.comments.get_leading(pos) {
        for comment in leading_comments.iter() {
          let comment_text = comment.text.trim();
          if self.ignore_statements.iter().any(|keyword| comment_text.trim() == keyword) {
            self.all_it_throws_comments.insert(comment.span);
          }
        }
      }
      
      // Also check trailing comments
      if let Some(trailing_comments) = self.comments.get_trailing(pos) {
        for comment in trailing_comments.iter() {
          let comment_text = comment.text.trim();
          if self.ignore_statements.iter().any(|keyword| comment_text.trim() == keyword) {
            self.all_it_throws_comments.insert(comment.span);
          }
        }
      }
    }
  }

  impl<'a> Visit for CommentCollector<'a> {
    fn visit_module(&mut self, module: &swc_ecma_ast::Module) {
      // Check comments at the module level
      self.check_comments_at_position(module.span.lo());
      swc_ecma_visit::visit_module(self, module);
    }

    fn visit_stmt(&mut self, stmt: &swc_ecma_ast::Stmt) {
      // Check comments at the statement level
      self.check_comments_at_position(stmt.span().lo());
      swc_ecma_visit::visit_stmt(self, stmt);
    }
    
    fn visit_expr(&mut self, expr: &swc_ecma_ast::Expr) {
      // Check comments at the expression level
      self.check_comments_at_position(expr.span().lo());
      swc_ecma_visit::visit_expr(self, expr);
    }
  }

  let mut collector = CommentCollector::new(ignore_statements, comments);
  collector.visit_module(module);
  let all_it_throws_comments: Vec<Span> = collector.all_it_throws_comments.into_iter().collect();

  // Step 2: Collect used comments from analyzers
  let mut used_comments = HashSet::new();
  
  // Collect from ThrowAnalyzer
  for used_comment in &throw_analyzer.used_it_throws_comments {
    used_comments.insert(*used_comment);
  }
  
  // Collect from CallFinder
  for used_comment in &call_finder.used_it_throws_comments {
    used_comments.insert(*used_comment);
  }

  // Step 3: Find unused comments
  let mut unused_comments = Vec::new();
  for comment_span in all_it_throws_comments {
    if !used_comments.contains(&comment_span) {
      unused_comments.push(comment_span);
    }
  }
  unused_comments
}


pub fn analyze_code(
  content: &str,
  cm: Lrc<SourceMap>,
  user_settings: &UserSettings,
) -> (AnalysisResult, Lrc<SourceMap>, Lrc<SingleThreadedComments>) {
  // Debug output removed for cleaner logs
  let fm = cm.new_source_file(swc_common::FileName::Anon, content.into());
  let comments = Lrc::new(SingleThreadedComments::default());
  let lexer = Lexer::new(
    Syntax::Typescript(swc_ecma_parser::TsConfig {
      tsx: true,
      decorators: true,
      dts: false,
      no_early_errors: false,
      disallow_ambiguous_jsx_like: false,
    }),
    EsVersion::latest(),
    StringInput::from(&*fm),
    Some(&comments),
  );

  let mut parser = Parser::new_from(lexer);
  let module = match parser.parse_module() {
    Ok(module) => module,
    Err(e) => {
      eprintln!("❌ Failed to parse module: {:?}", e);
      // Return empty analysis result on parse failure
      return (AnalysisResult::default(), cm, Lrc::new(SingleThreadedComments::default()));
    }
  };
  // Create and populate type registry from JSDoc definitions
  let mut callback_finder = CallbackFinder::new(comments.clone());
  callback_finder.analyze_module(&module);
  
  let mut typedef_finder = TypedefFinder::new(comments.clone());
  typedef_finder.analyze_module(&module);
  
  let mut param_finder = ParamFinder::new(comments.clone());
  param_finder.visit_module(&module);
  
  // Build type registry from callback and typedef definitions
  let mut type_registry = TypeRegistry::new();
  
  // Add callback definitions
  for (name, callback_def) in callback_finder.get_all_callbacks() {
    type_registry.callbacks.insert(name.clone(), callback_def.clone());
  }
  
  // Add typedef definitions (including callback typedefs)
  for (name, typedef_def) in typedef_finder.get_all_typedefs() {
    type_registry.typedefs.insert(name.clone(), typedef_def.clone());
  }

  let mut throw_collector = ThrowAnalyzer {
    comments: comments.clone(),
    functions_with_throws: HashSet::new(),
    json_parse_calls: vec![],
    fs_access_calls: vec![],
    import_sources: HashSet::new(),
    imported_identifiers: Vec::new(),
    function_name_stack: vec![],
    current_class_name: None,
    current_method_name: None,
    throwfinder_settings: ThrowFinderSettings {
      ignore_statements: &user_settings.ignore_statements.clone(),
      include_try_statements: &user_settings.include_try_statement_throws.clone(),
    },
    used_it_throws_comments: HashSet::new(),
    type_registry,
  };
  throw_collector.visit_module(&module);
  
  let mut call_collector = CallFinder::new(comments.clone());
  call_collector.functions_with_throws = throw_collector.functions_with_throws.clone();
  call_collector.visit_module(&module);

  // Mark function-level @it-throws comments as used
  throw_collector.mark_function_it_throws_comments_as_used();

  // Find all @it-throws comments and determine which are unused
  let unused_comments = find_unused_it_throws_comments(
    &comments,
    &module,
    &user_settings.ignore_statements,
    &throw_collector,
    &call_collector,
  );

  let mut import_usages_collector = ImportUsageFinder {
    imported_identifiers: throw_collector.imported_identifiers.clone(),
    imported_identifier_usages: HashSet::new(),
    current_class_name: None,
    current_method_name: None,
    function_name_stack: vec![],
  };
  import_usages_collector.visit_module(&module);

  // Build a map of callback typedef names -> their throws types for parameter mapping
  let callback_type_throws: std::collections::HashMap<String, Vec<String>> = typedef_finder
    .get_callback_typedefs()
    .into_iter()
    .filter_map(|(name, def)| def.throws_annotation.as_ref().map(|ann| (name, ann.error_types.clone())))
    .collect();

  let mut function_collector = FunctionFinder::new(comments.clone()).with_callback_types(callback_type_throws);
  function_collector.visit_module(&module);
  // Pass parameter-level throws metadata from function finder to call finder
  call_collector.param_throws = function_collector.param_throws.clone();
  
  // Integrate parameter throws information from the new param finder
  // This provides more detailed parameter-level @throws analysis
  for (function_id, param_throws_list) in param_finder.param_throws.iter() {
    // Convert ParamThrowsInfo to the format expected by CallFinder
    let mut param_throws_vec: Vec<Vec<String>> = Vec::new();
    
    // Initialize with empty vectors for all parameters
    if let Some(existing_params) = call_collector.param_throws.get(function_id) {
      param_throws_vec = existing_params.clone();
    }
    
    // Update with new parameter throws information
    for param_info in param_throws_list {
      // Ensure we have enough slots for this parameter index
      while param_throws_vec.len() <= param_info.param_index {
        param_throws_vec.push(Vec::new());
      }
      
      // Add the throws information for this parameter
      param_throws_vec[param_info.param_index] = param_info.throws_annotation.error_types.clone();
    }
    
    call_collector.param_throws.insert(function_id.clone(), param_throws_vec);
  }
  
  println!("🔧 Registered functions:");
  for func in &function_collector.functions {
    println!("  - {} ({})", func.name, func.id);
  }

  // Create and populate catch analyses with actual thrown errors
  let mut try_catch_finder = TryCatchFinder::new(comments.clone());
  try_catch_finder.visit_module(&module);
  
  // Populate catch analyses with actual thrown errors found by ThrowFinder
  let populated_catch_analyses = populate_catch_analyses_with_throws(
    try_catch_finder.all_catches, 
    &throw_collector.functions_with_throws,
    &module,
  );
  
  println!("🔧 Catch analysis populated:");
  for (i, catch_analysis) in populated_catch_analyses.iter().enumerate() {
    println!("  [{}] Try block has {} thrown errors: {:?}", 
      i, 
      catch_analysis.errors_thrown_in_try.len(),
      catch_analysis.errors_thrown_in_try
    );
    println!("  [{}] Catch handles: {:?}", i, catch_analysis.errors_handled_in_catch);
    println!("  [{}] Effectively caught: {:?}", i, catch_analysis.errors_effectively_caught);
    println!("  [{}] Propagated: {:?}", i, catch_analysis.errors_propagated);
  }

  // First, get the preliminary filtered functions (before propagation)
  let preliminary_filtered_functions = {
    let mut filtered = HashSet::new();
    for throw_map in &throw_collector.functions_with_throws {
      if let Some(function_info) = function_collector.functions.iter().find(|f| f.id == throw_map.id) {
        let effectively_caught_errors = get_effectively_caught_errors_for_function(
          function_info.span, 
          &populated_catch_analyses
        );
        
        let mut filtered_throw_details = throw_map.throw_details.clone();
        filtered_throw_details.retain(|throw_detail| {
          if let Some(ref error_type) = throw_detail.error_type {
            !effectively_caught_errors.contains(error_type)
          } else {
            true
          }
        });
        
        if !filtered_throw_details.is_empty() {
          filtered.insert(throw_map.id.clone());
        }
      } else {
        filtered.insert(throw_map.id.clone());
      }
    }
    filtered
  };

  // Handle different logic based on include_try_statement_throws setting
  let (final_functions_with_throws, filtered_calls_to_throws) = if user_settings.include_try_statement_throws {
    // When including try statement throws, use original calls and simple propagation
    let final_functions = propagate_throws_to_callers_without_catch_filtering(
      throw_collector.functions_with_throws,
      &call_collector.calls,
      &function_collector.functions,
    );
    (final_functions, call_collector.calls)
  } else {
    // When excluding try statement throws, filter calls and use enhanced catch analysis
    let filtered_calls = filter_calls_through_catch_analysis_and_function_availability(
      call_collector.calls,
      &function_collector.functions,
      &populated_catch_analyses,
      &preliminary_filtered_functions,
    );
    
    let final_functions = propagate_throws_to_callers(
      throw_collector.functions_with_throws,
      &filtered_calls,
      &function_collector.functions,
      &populated_catch_analyses,
    );
    
    (final_functions, filtered_calls)
  };

  // Apply include_try_statements setting: filter out throws that are within try blocks
  // if the user has disabled include_try_statements
  let final_functions_with_throws = if user_settings.include_try_statement_throws {
    println!("🔧 include_try_statement_throws is true, keeping all {} functions with throws", final_functions_with_throws.len());
    final_functions_with_throws
  } else {
    println!("🔧 include_try_statement_throws is false, filtering {} functions with throws", final_functions_with_throws.len());
    let filtered = filter_functions_exclude_try_block_throws(
      final_functions_with_throws,
      &function_collector.functions,
      &populated_catch_analyses,
    );
    println!("🔧 After filtering: {} functions remain", filtered.len());
    for func in &filtered {
      println!("  - Remaining function: {} ({})", func.function_or_method_name, func.id);
    }
    filtered
  };

  println!("🔧 Final result summary:");  
  println!("  - functions_with_throws: {}", final_functions_with_throws.len());
  println!("  - calls_to_throws: {}", filtered_calls_to_throws.len());
  
  (AnalysisResult {
    functions_with_throws: final_functions_with_throws,
    calls_to_throws: filtered_calls_to_throws, // Use filtered calls instead of raw calls
    json_parse_calls: throw_collector.json_parse_calls,
    fs_access_calls: throw_collector.fs_access_calls,
    import_sources: throw_collector.import_sources,
    imported_identifiers: throw_collector.imported_identifiers,
    imported_identifier_usages: import_usages_collector.imported_identifier_usages,
    catch_analyses: populated_catch_analyses, // Use populated catch analyses
    unused_it_throws_comments: unused_comments,
    all_functions: function_collector.functions, // Include all functions for JSDoc checking
    inline_callback_allowed_throws: call_collector.inline_callback_allowed_throws, // Pass inline callback allowed throws
  }, cm, comments)
}
