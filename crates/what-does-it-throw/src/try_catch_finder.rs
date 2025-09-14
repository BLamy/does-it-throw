extern crate swc_common;
extern crate swc_ecma_ast;
extern crate swc_ecma_visit;

use swc_ecma_ast::{
  BinaryOp, TryStmt, BlockStmt, Expr, Pat, ThrowStmt,
  BinExpr,
};

use self::swc_common::{comments::Comments, sync::Lrc, Span};
use self::swc_ecma_visit::{Visit, VisitWith};


/// Represents the analysis of a single catch block
#[derive(Clone, Debug)]
pub struct CatchAnalysis {
    pub catch_span: Span,
    pub try_span: Span,
    pub catch_param: Option<String>,
    pub errors_thrown_in_try: Vec<String>,
    pub errors_handled_in_catch: Vec<String>, // instanceof checks found
    pub errors_rethrown_in_catch: Vec<String>, // throw statements in catch
    pub errors_effectively_caught: Vec<String>, // handled but not re-thrown
    pub errors_propagated: Vec<String>, // re-thrown or not handled
    pub has_escape_hatch: bool, // true if `throw e` (catch param) is used
    pub missing_handlers: Vec<String>, // error types that need instanceof checks
}

impl CatchAnalysis {
    pub fn new(catch_span: Span, try_span: Span, catch_param: Option<String>) -> Self {
        Self {
            catch_span,
            try_span,
            catch_param,
            errors_thrown_in_try: Vec::new(),
            errors_handled_in_catch: Vec::new(),
            errors_rethrown_in_catch: Vec::new(),
            errors_effectively_caught: Vec::new(),
            errors_propagated: Vec::new(),
            has_escape_hatch: false,
            missing_handlers: Vec::new(),
        }
    }

    pub fn add_thrown_error(&mut self, error_type: String) {
        if !self.errors_thrown_in_try.contains(&error_type) {
            self.errors_thrown_in_try.push(error_type);
        }
    }

    pub fn add_handled_error(&mut self, error_type: String) {
        if !self.errors_handled_in_catch.contains(&error_type) {
            self.errors_handled_in_catch.push(error_type);
        }
    }

    pub fn add_rethrown_error(&mut self, error_type: String) {
        if !self.errors_rethrown_in_catch.contains(&error_type) {
            self.errors_rethrown_in_catch.push(error_type);
        }
    }

    pub fn set_escape_hatch(&mut self, has_escape: bool) {
        self.has_escape_hatch = has_escape;
    }

    /// Calculate which errors are missing handlers and which are effectively caught
    pub fn calculate_error_flow(&mut self) {
        self.missing_handlers.clear();
        self.errors_effectively_caught.clear();
        self.errors_propagated.clear();

        // If there are no instanceof checks and no escape hatch, this is a simple catch block
        // that catches all errors by default
        let is_simple_catch_all = self.errors_handled_in_catch.is_empty() && !self.has_escape_hatch;

        for error_type in &self.errors_thrown_in_try {
            let is_handled = self.errors_handled_in_catch.contains(error_type);
            let is_specifically_rethrown = self.errors_rethrown_in_catch.contains(error_type);

            if is_simple_catch_all && !is_specifically_rethrown {
                // Simple catch block with no instanceof checks - catches all errors
                self.errors_effectively_caught.push(error_type.clone());
            } else if is_handled && !is_specifically_rethrown && !self.has_escape_hatch {
                // Error is handled with instanceof and not re-thrown
                self.errors_effectively_caught.push(error_type.clone());
            } else if is_handled && self.has_escape_hatch {
                // Error is handled but escape hatch exists - it's effectively caught
                self.errors_effectively_caught.push(error_type.clone());
            } else if !is_handled && self.has_escape_hatch {
                // Error is not handled but escape hatch exists - it propagates
                self.errors_propagated.push(error_type.clone());
            } else if !is_handled && !is_simple_catch_all {
                // Error is not handled and no escape hatch - missing handler!
                self.missing_handlers.push(error_type.clone());
                self.errors_propagated.push(error_type.clone());
            } else {
                // Error is handled but specifically re-thrown
                self.errors_propagated.push(error_type.clone());
            }
        }
    }

    /// Check if this catch block is exhaustive (handles all errors or has escape hatch)
    pub fn is_exhaustive(&self) -> bool {
        self.missing_handlers.is_empty()
    }

    /// Check if this catch block has validation errors
    pub fn has_validation_errors(&self) -> bool {
        !self.missing_handlers.is_empty()
    }
}

/// Visitor to find and analyze try-catch blocks
pub struct TryCatchFinder {
    pub comments: Lrc<dyn Comments>,
    pub all_catches: Vec<CatchAnalysis>,
    current_try_block: Option<Span>,
    current_catch_analysis: Option<CatchAnalysis>,
}

impl TryCatchFinder {
    pub fn new(comments: Lrc<dyn Comments>) -> Self {
        Self {
            comments,
            all_catches: Vec::new(),
            current_try_block: None,
            current_catch_analysis: None,
        }
    }

    fn extract_catch_param(&self, param: &Option<Pat>) -> Option<String> {
        if let Some(param) = param {
            match param {
                Pat::Ident(ident) => Some(ident.id.sym.to_string()),
                _ => None,
            }
        } else {
            None
        }
    }

    fn find_instanceof_checks(&self, catch_block: &BlockStmt) -> Vec<String> {
        let mut visitor = InstanceOfVisitor::new();
        catch_block.visit_with(&mut visitor);
        visitor.error_types
    }

    fn find_rethrows_in_catch(&self, catch_block: &BlockStmt, catch_param: &Option<String>) -> (Vec<String>, bool) {
        let mut visitor = RethrowVisitor::new(catch_param.clone());
        catch_block.visit_with(&mut visitor);
        (visitor.rethrown_types, visitor.has_escape_hatch)
    }


    // This would be populated by integration with ThrowFinder
    fn analyze_throws_in_try_block(&self, _try_block: &BlockStmt) -> Vec<String> {
        // In real implementation, this would use ThrowFinder to get actual thrown errors
        // For now, return empty - this gets populated by the main analysis pipeline
        Vec::new()
    }
}

impl Visit for TryCatchFinder {
    fn visit_try_stmt(&mut self, try_stmt: &TryStmt) {
        self.current_try_block = Some(try_stmt.span);

        // Analyze each catch handler (there's typically only one in JS/TS)
        if let Some(catch_clause) = &try_stmt.handler {
            let catch_param = self.extract_catch_param(&catch_clause.param);
            let mut catch_analysis = CatchAnalysis::new(
                catch_clause.span,
                try_stmt.block.span,
                catch_param.clone(),
            );

            // Find instanceof checks in the catch block
            let handled_errors = self.find_instanceof_checks(&catch_clause.body);
            for error_type in handled_errors {
                catch_analysis.add_handled_error(error_type);
            }

            // Find re-throws in the catch block
            let (rethrown_errors, has_escape_hatch) = self.find_rethrows_in_catch(&catch_clause.body, &catch_param);
            catch_analysis.set_escape_hatch(has_escape_hatch);
            for error_type in rethrown_errors {
                catch_analysis.add_rethrown_error(error_type);
            }

            // Analyze throws in the try block (would be populated by ThrowFinder integration)
            let thrown_errors = self.analyze_throws_in_try_block(&try_stmt.block);
            for error_type in thrown_errors {
                catch_analysis.add_thrown_error(error_type);
            }

            // Calculate error flow
            catch_analysis.calculate_error_flow();

            self.current_catch_analysis = Some(catch_analysis.clone());
            self.all_catches.push(catch_analysis);
        }

        // Continue visiting child nodes
        try_stmt.visit_children_with(self);

        self.current_try_block = None;
        self.current_catch_analysis = None;
    }
}

struct InstanceOfVisitor {
    error_types: Vec<String>,
}

impl InstanceOfVisitor {
    fn new() -> Self {
        Self {
            error_types: Vec::new(),
        }
    }
}

impl Visit for InstanceOfVisitor {
    fn visit_bin_expr(&mut self, bin_expr: &BinExpr) {
        if matches!(bin_expr.op, BinaryOp::InstanceOf) {
            if let Expr::Ident(ident) = &*bin_expr.right {
                let error_type = ident.sym.to_string();
                if !self.error_types.contains(&error_type) {
                    self.error_types.push(error_type);
                }
            }
        }
        bin_expr.visit_children_with(self);
    }
}

struct RethrowVisitor {
    catch_param: Option<String>,
    rethrown_types: Vec<String>,
    has_escape_hatch: bool,
}

impl RethrowVisitor {
    fn new(catch_param: Option<String>) -> Self {
        Self {
            catch_param,
            rethrown_types: Vec::new(),
            has_escape_hatch: false,
        }
    }
}

impl Visit for RethrowVisitor {
    fn visit_throw_stmt(&mut self, throw_stmt: &ThrowStmt) {
        match &*throw_stmt.arg {
            Expr::New(new_expr) => {
                if let Expr::Ident(ident) = &*new_expr.callee {
                    let error_type = ident.sym.to_string();
                    if !self.rethrown_types.contains(&error_type) {
                        self.rethrown_types.push(error_type);
                    }
                }
            }
            Expr::Ident(ident) => {
                let var_name = ident.sym.to_string();
                if let Some(ref catch_param) = self.catch_param {
                    if var_name == *catch_param {
                        // This is `throw e` where e is the catch parameter - escape hatch!
                        self.has_escape_hatch = true;
                    }
                }
                if !self.rethrown_types.contains(&format!("variable: {}", var_name)) {
                    self.rethrown_types.push(format!("variable: {}", var_name));
                }
            }
            _ => {}
        }
        throw_stmt.visit_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swc_common::comments::SingleThreadedComments;
    use swc_common::{sync::Lrc, FileName, SourceMap};
    use swc_ecma_ast::Module;
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

        let module = parser.parse_module().expect("Failed to parse module");
        (module, comments)
    }

    #[test]
    fn test_exhaustive_by_default() {
        let code = r#"
            try {
                // Errors would be detected by ThrowFinder
            } catch (e) {
                if (e instanceof ValidationError) {
                    console.log("handled");
                    return null;
                } else if (e instanceof NetworkError) {
                    console.log("handled");
                    return null;
                }
                // Missing handlers - should be validation error
            }
        "#;

        let (module, comments) = parse_code_with_comments(code);
        let mut finder = TryCatchFinder::new(comments);
        finder.visit_module(&module);

        assert_eq!(finder.all_catches.len(), 1);
        
        let catch_analysis = &finder.all_catches[0];
        assert_eq!(catch_analysis.errors_handled_in_catch, vec!["ValidationError", "NetworkError"]);
        assert!(!catch_analysis.has_escape_hatch);
        
        // Simulate thrown errors (normally from ThrowFinder)
        let mut analysis = catch_analysis.clone();
        analysis.add_thrown_error("ValidationError".to_string());
        analysis.add_thrown_error("NetworkError".to_string()); 
        analysis.add_thrown_error("AuthError".to_string());
        analysis.calculate_error_flow();
        
        // Should have missing handler for AuthError
        assert_eq!(analysis.missing_handlers, vec!["AuthError"]);
        assert!(!analysis.is_exhaustive());
    }

    #[test]
    fn test_escape_hatch_with_throw_e() {
        let code = r#"
            try {
                // Errors would be detected by ThrowFinder
            } catch (e) {
                if (e instanceof ValidationError) {
                    console.log("handled");
                    return null;
                }
                throw e; // Escape hatch - explicitly re-throw unhandled
            }
        "#;

        let (module, comments) = parse_code_with_comments(code);
        let mut finder = TryCatchFinder::new(comments);
        finder.visit_module(&module);

        assert_eq!(finder.all_catches.len(), 1);
        
        let catch_analysis = &finder.all_catches[0];
        assert_eq!(catch_analysis.errors_handled_in_catch, vec!["ValidationError"]);
        assert!(catch_analysis.has_escape_hatch);
        
        // Simulate thrown errors
        let mut analysis = catch_analysis.clone();
        analysis.add_thrown_error("ValidationError".to_string());
        analysis.add_thrown_error("NetworkError".to_string());
        analysis.calculate_error_flow();
        
        // With escape hatch, ValidationError is effectively caught, NetworkError propagates
        assert_eq!(analysis.errors_effectively_caught, vec!["ValidationError"]);
        assert_eq!(analysis.errors_propagated, vec!["NetworkError"]);
        assert!(analysis.is_exhaustive()); // No missing handlers because of escape hatch
    }

    #[test]
    fn test_specific_rethrow() {
        let code = r#"
            try {
                // Errors would be detected by ThrowFinder  
            } catch (e) {
                if (e instanceof ValidationError) {
                    console.log("handled");
                    return null;
                } else if (e instanceof NetworkError) {
                    console.log("network error");
                    throw new NetworkError("Enhanced: " + e.message); // Specific re-throw
                }
                // AuthError not handled, no escape hatch - validation error
            }
        "#;

        let (module, comments) = parse_code_with_comments(code);
        let mut finder = TryCatchFinder::new(comments);
        finder.visit_module(&module);

        assert_eq!(finder.all_catches.len(), 1);
        
        let catch_analysis = &finder.all_catches[0];
        assert_eq!(catch_analysis.errors_handled_in_catch, vec!["ValidationError", "NetworkError"]);
        assert_eq!(catch_analysis.errors_rethrown_in_catch, vec!["NetworkError"]);
        assert!(!catch_analysis.has_escape_hatch);
        
        // Simulate thrown errors
        let mut analysis = catch_analysis.clone();
        analysis.add_thrown_error("ValidationError".to_string());
        analysis.add_thrown_error("NetworkError".to_string());
        analysis.add_thrown_error("AuthError".to_string());
        analysis.calculate_error_flow();
        
        // ValidationError effectively caught, NetworkError and AuthError propagate
        assert_eq!(analysis.errors_effectively_caught, vec!["ValidationError"]);
        assert_eq!(analysis.errors_propagated, vec!["NetworkError", "AuthError"]);
        assert_eq!(analysis.missing_handlers, vec!["AuthError"]); // Missing handler
        assert!(!analysis.is_exhaustive());
    }

    #[test]
    fn test_complete_exhaustive_catch() {
        let code = r#"
            try {
                // Errors would be detected by ThrowFinder
            } catch (e) {
                if (e instanceof ValidationError) {
                    return handleValidation(e);
                } else if (e instanceof NetworkError) {
                    return handleNetwork(e);
                } else if (e instanceof AuthError) {
                    return handleAuth(e);
                }
                // All errors handled - should be exhaustive
            }
        "#;

        let (module, comments) = parse_code_with_comments(code);
        let mut finder = TryCatchFinder::new(comments);
        finder.visit_module(&module);

        assert_eq!(finder.all_catches.len(), 1);
        
        let catch_analysis = &finder.all_catches[0];
        assert_eq!(catch_analysis.errors_handled_in_catch, vec!["ValidationError", "NetworkError", "AuthError"]);
        assert!(!catch_analysis.has_escape_hatch);
        
        // Simulate thrown errors matching handlers
        let mut analysis = catch_analysis.clone();
        analysis.add_thrown_error("ValidationError".to_string());
        analysis.add_thrown_error("NetworkError".to_string());
        analysis.add_thrown_error("AuthError".to_string());
        analysis.calculate_error_flow();
        
        // All errors effectively caught
        assert_eq!(analysis.errors_effectively_caught, vec!["ValidationError", "NetworkError", "AuthError"]);
        assert!(analysis.errors_propagated.is_empty());
        assert!(analysis.missing_handlers.is_empty());
        assert!(analysis.is_exhaustive());
    }
} 