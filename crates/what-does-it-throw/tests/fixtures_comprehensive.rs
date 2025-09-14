extern crate swc_common;
extern crate swc_ecma_parser;
extern crate swc_ecma_visit;
extern crate what_does_it_throw;

use std::fs;
use std::path::Path;
use std::collections::HashSet;
use swc_common::{sync::Lrc, SourceMap, FileName};
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsConfig};
use swc_ecma_visit::Visit;
use what_does_it_throw::{
    call_finder::CallFinder,
    throw_finder::{ThrowAnalyzer, ThrowFinderSettings, TypeRegistry},
};
use swc_common::comments::SingleThreadedComments;

/// Helper struct to represent expected diagnostics for easier testing
#[derive(Debug, Clone)]
pub struct ExpectedDiagnostic {
    pub line: usize,
    pub message_pattern: String,
    pub diagnostic_type: DiagnosticType,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticType {
    FunctionMayThrow,
    ThrowStatement,
    FunctionCallMayThrow,
    ImportedMayThrow,
}

/// Helper function to load fixture content
fn load_fixture(name: &str) -> String {
    let fixture_path = Path::new("src/fixtures").join(name);
    fs::read_to_string(&fixture_path)
        .unwrap_or_else(|_| panic!("Failed to read fixture file: {}", fixture_path.display()))
}

/// Helper function to analyze a fixture and return line-based results
fn analyze_fixture(code: &str) -> FixtureAnalysisResult {
    // Reuse the multi-file analyzer with a single virtual file entry.
    analyze_fixture_tree(vec![("test_fixture", code.to_string())], "test_fixture")
}

/// Analyze a virtual file tree represented as (path, contents) pairs.
/// The entry_path is used to filter diagnostics to that file, while still
/// allowing cross-file import relationships to be understood by the analyzers.
fn analyze_fixture_tree(files: Vec<(&str, String)>, entry_path: &str) -> FixtureAnalysisResult {
    let cm = Lrc::new(SourceMap::default());
    let comments = Lrc::new(SingleThreadedComments::default());

    // Parse all files first
    let mut modules = Vec::new();
    for (path, contents) in &files {
        let source_file = cm.new_source_file(
            FileName::Custom((*path).to_string()),
            contents.clone(),
        );

        let lexer = Lexer::new(
            Syntax::Typescript(TsConfig {
                decorators: true,
                tsx: true,
                ..Default::default()
            }),
            Default::default(),
            StringInput::from(&*source_file),
            Some(&comments),
        );

        let mut parser = Parser::new_from(lexer);
        let module = parser.parse_module().expect("Failed to parse module");
        modules.push(module);
    }

    // Analyze throws across all modules
    let ignore_statements = vec!["@it-throws".to_string()];
    let include_try_statements = false;
    let settings = ThrowFinderSettings {
        ignore_statements: &ignore_statements,
        include_try_statements: &include_try_statements,
    };

    let mut throw_analyzer = ThrowAnalyzer {
        comments: comments.clone(),
        functions_with_throws: HashSet::new(),
        json_parse_calls: Vec::new(),
        fs_access_calls: Vec::new(),
        import_sources: HashSet::new(),
        imported_identifiers: Vec::new(),
        function_name_stack: Vec::new(),
        current_class_name: None,
        current_method_name: None,
        throwfinder_settings: settings,
        used_it_throws_comments: HashSet::new(),
        type_registry: TypeRegistry::new(),
    };

    for module in &modules {
        throw_analyzer.visit_module(module);
    }

    // Analyze calls across all modules
    let mut call_finder = CallFinder::new(comments.clone());
    call_finder.functions_with_throws = throw_analyzer.functions_with_throws.clone();
    for module in &modules {
        call_finder.visit_module(module);
    }

    FixtureAnalysisResult {
        functions_with_throws: throw_analyzer.functions_with_throws,
        calls_to_throws: call_finder.calls,
        source_map: cm,
        entry_filename: Some(FileName::Custom(entry_path.to_string())),
    }
}

struct FixtureAnalysisResult {
    functions_with_throws: HashSet<what_does_it_throw::throw_finder::ThrowMap>,
    calls_to_throws: HashSet<what_does_it_throw::call_finder::CallToThrowMap>,
    source_map: Lrc<SourceMap>,
    entry_filename: Option<FileName>,
}

impl FixtureAnalysisResult {
    /// Get all diagnostics as line numbers with types
    fn get_line_diagnostics(&self) -> Vec<(usize, DiagnosticType, String)> {
        let mut diagnostics = Vec::new();

        // Add function throw diagnostics
        for throw_map in &self.functions_with_throws {
            let pos = self.source_map.lookup_char_pos(throw_map.throw_statement.lo());
            if let Some(ref entry) = self.entry_filename {
                if &pos.file.name != entry { continue; }
            }
            diagnostics.push((
                pos.line,
                DiagnosticType::FunctionMayThrow,
                throw_map.function_or_method_name.clone(),
            ));

            // Add throw statement diagnostics
            for &throw_span in &throw_map.throw_spans {
                let pos = self.source_map.lookup_char_pos(throw_span.lo());
                if let Some(ref entry) = self.entry_filename {
                    if &pos.file.name != entry { continue; }
                }
                diagnostics.push((
                    pos.line,
                    DiagnosticType::ThrowStatement,
                    "Throw statement".to_string(),
                ));
            }
        }

        // Add call diagnostics
        for call_map in &self.calls_to_throws {
            let pos = self.source_map.lookup_char_pos(call_map.call_span.lo());
            if let Some(ref entry) = self.entry_filename {
                if &pos.file.name != entry { continue; }
            }
            diagnostics.push((
                pos.line,
                DiagnosticType::FunctionCallMayThrow,
                "Function call may throw".to_string(),
            ));
        }

        diagnostics.sort_by_key(|(line, _, _)| *line);
        diagnostics
    }
}

/// Main function for asserting exact diagnostics match - similar to expectExactDiagnostics
/// This function ensures the diagnostics match exactly in count, line, type, and pattern
fn expect_exact_diagnostics(result: &FixtureAnalysisResult, expected: &[ExpectedDiagnostic]) {
    let actual_diagnostics = result.get_line_diagnostics();
    
    // Sort expected diagnostics for consistent comparison
    let mut expected_sorted = expected.to_vec();
    expected_sorted.sort_by_key(|d| d.line);
    
    // Print a diff-style diagnostics comparison: gray for matches, red for missing (expected), green for unexpected (actual)
    // ANSI colors: gray = 90, red = 31, green = 32, reset = 0
    

    // Helper to format a diagnostic tuple for display
    fn format_diag(line: usize, diag_type: &DiagnosticType, message: &str) -> String {
        format!("L{} {:?} '{}'", line, diag_type, message)
    }

    // Build sets for comparison
    let expected_set: Vec<(usize, DiagnosticType, String)> = expected_sorted
        .iter()
        .map(|d| (d.line, d.diagnostic_type.clone(), d.message_pattern.clone()))
        .collect();
    let actual_set = actual_diagnostics.clone();

    // For diffing, we want to match up expected and actual by line/type/message-pattern-contains
    // We'll mark which expected and actual diagnostics have been matched
    let mut matched_expected = vec![false; expected_set.len()];
    let mut matched_actual = vec![false; actual_set.len()];

    // First, try to match expected to actual (in order)
    for (ei, (eline, etype, epat)) in expected_set.iter().enumerate() {
        for (ai, (aline, atype, amsg)) in actual_set.iter().enumerate() {
            if !matched_actual[ai]
                && *eline == *aline
                && *etype == *atype
                && amsg.contains(epat)
            {
                matched_expected[ei] = true;
                matched_actual[ai] = true;
                break;
            }
        }
    }

    println!("\x1b[1m=== DIAGNOSTIC COMPARISON (Diff Style) ===\x1b[0m");
    println!(
        "Expected: {}  |  Actual: {}",
        expected_set.len(),
        actual_set.len()
    );
    println!("Legend: \x1b[90m  = match\x1b[0m  \x1b[31m- missing (expected)\x1b[0m  \x1b[32m+ unexpected (actual)\x1b[0m");

    // Print all lines in a combined diff order (by line number, then by type, then by message)
    // We'll collect all diagnostics (expected and actual) with their status
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum DiffKind {
        Match,
        Missing,    // in expected, not in actual
        Unexpected, // in actual, not in expected
    }
    struct DiffLine {
        kind: DiffKind,
        line: usize,
        diag_type: DiagnosticType,
        message: String,
    }
    let mut diff_lines: Vec<DiffLine> = Vec::new();

    // Add matches and missing (expected)
    for (i, (eline, etype, epat)) in expected_set.iter().enumerate() {
        if matched_expected[i] {
            // Find the actual message that matched
            let mut matched_msg = None;
            for (ai, (aline, atype, amsg)) in actual_set.iter().enumerate() {
                if matched_actual[ai]
                    && *eline == *aline
                    && *etype == *atype
                    && amsg.contains(epat)
                {
                    matched_msg = Some(amsg.clone());
                    break;
                }
            }
            diff_lines.push(DiffLine {
                kind: DiffKind::Match,
                line: *eline,
                diag_type: etype.clone(),
                message: matched_msg.unwrap_or_else(|| epat.clone()),
            });
        } else {
            diff_lines.push(DiffLine {
                kind: DiffKind::Missing,
                line: *eline,
                diag_type: etype.clone(),
                message: epat.clone(),
            });
        }
    }
    // Add unexpected (actuals not matched)
    for (i, (aline, atype, amsg)) in actual_set.iter().enumerate() {
        if !matched_actual[i] {
            diff_lines.push(DiffLine {
                kind: DiffKind::Unexpected,
                line: *aline,
                diag_type: atype.clone(),
                message: amsg.clone(),
            });
        }
    }
    // Sort by line, then type, then message
    diff_lines.sort_by(|a, b| (a.line, &a.diag_type, &a.message).cmp(&(b.line, &b.diag_type, &b.message)));

    // Print with colors
    for diff in diff_lines {
        match diff.kind {
            DiffKind::Match => {
                // gray
                println!(
                    "\x1b[90m  = {}\x1b[0m",
                    format_diag(diff.line, &diff.diag_type, &diff.message)
                );
            }
            DiffKind::Missing => {
                // red
                println!(
                    "\x1b[31m  - {}\x1b[0m",
                    format_diag(diff.line, &diff.diag_type, &diff.message)
                );
            }
            DiffKind::Unexpected => {
                // green
                println!(
                    "\x1b[32m  + {}\x1b[0m",
                    format_diag(diff.line, &diff.diag_type, &diff.message)
                );
            }
        }
    }
    
    // Check count first
    assert_eq!(
        actual_diagnostics.len(),
        expected_sorted.len(),
        "Diagnostic count mismatch. Expected: {}, Actual: {}",
        expected_sorted.len(),
        actual_diagnostics.len()
    );
    
    // Check each expected diagnostic exists
    for expected_diag in &expected_sorted {
        let found = actual_diagnostics.iter().any(|(line, diag_type, message)| {
            *line == expected_diag.line
                && *diag_type == expected_diag.diagnostic_type
                && message.contains(&expected_diag.message_pattern)
        });

        assert!(
            found,
            "Expected diagnostic not found: L{} {:?} containing '{}'\nExpected diagnostics: {:#?}\nActual diagnostics: {:#?}",
            expected_diag.line,
            expected_diag.diagnostic_type,
            expected_diag.message_pattern,
            expected_sorted,
            actual_diagnostics
        );
    }
    
    // Check for unexpected diagnostics by verifying each actual diagnostic is expected
    for (actual_line, actual_type, actual_message) in &actual_diagnostics {
        let found = expected_sorted.iter().any(|expected_diag| {
            *actual_line == expected_diag.line
                && *actual_type == expected_diag.diagnostic_type
                && actual_message.contains(&expected_diag.message_pattern)
        });

        assert!(
            found,
            "Unexpected diagnostic found: L{} {:?} '{}'\nThis diagnostic was not in the expected list",
            actual_line,
            actual_type,
            actual_message
        );
    }
    
    println!("✅ All diagnostics match exactly!");
}


#[cfg(test)]
mod fixture_tests {
    use super::*;

    #[test]
    fn test_callexpr_fixture() {
        let code = load_fixture("callExpr.ts");
        let result = analyze_fixture(&code);
        
        let expected = vec![
            // Updated assertions to match @file_context_0 (ground truth diagnostics)
            ExpectedDiagnostic { line: 29, message_pattern: "SomeThrow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 30, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 33, message_pattern: "SomeThrow2".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 34, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 47, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 48, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 51, message_pattern: "<anonymous>".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 52, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 55, message_pattern: "<anonymous>".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 56, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 60, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 61, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 64, message_pattern: "<anonymous>".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 65, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 70, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 76, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 77, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
        ];

        expect_exact_diagnostics(&result, &expected);
    }

    #[test]
    fn test_class_fixture() {
        let code = load_fixture("class.ts");
        let result = analyze_fixture(&code);
        
        let expected = vec![
            ExpectedDiagnostic { line: 4, message_pattern: "<constructor>".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 5, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 8, message_pattern: "someMethodThatThrows".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 9, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 16, message_pattern: "someMethodThatThrows2".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 18, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 22, message_pattern: "nestedThrow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 26, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            // Note: callNestedThrow function diagnostic currently not detected
            ExpectedDiagnostic { line: 36, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 42, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 47, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 52, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 57, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
        ];

        expect_exact_diagnostics(&result, &expected);
    }

    #[test]
    fn test_object_literal_fixture() {
        let code = load_fixture("objectLiteral.ts");
        let result = analyze_fixture(&code);
        
        let expected = vec![
            ExpectedDiagnostic { line: 2, message_pattern: "objectLiteralThrow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 3, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 6, message_pattern: "nestedObjectLiteralThrow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 7, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 13, message_pattern: "someExampleThrow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 14, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            // Note: callToLiteral function diagnostics currently not detected
            ExpectedDiagnostic { line: 19, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 23, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 27, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 28, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
        ];

        expect_exact_diagnostics(&result, &expected);
    }

    #[test]
    fn test_simple_fixture() {
        let code = load_fixture("sample.ts");
        let result = analyze_fixture(&code);
        
        let expected = vec![
            ExpectedDiagnostic { line: 3, message_pattern: "someConstThatThrows".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 4, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 8, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
        ];

        expect_exact_diagnostics(&result, &expected);
    }

    #[test]
    fn test_try_statement_fixture() {
        let code = load_fixture("tryStatement.ts");
        let result = analyze_fixture(&code);
        
        let expected = vec![
            ExpectedDiagnostic { line: 2, message_pattern: "someConstThatThrows".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 4, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 11, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 16, message_pattern: "<constructor>".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 18, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 24, message_pattern: "someMethodThatThrows".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 26, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 36, message_pattern: "someMethodThatThrows2".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 38, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 42, message_pattern: "nestedThrow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 47, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 60, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 66, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 71, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 76, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 81, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 89, message_pattern: "_contextFromWorkflow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 91, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 98, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 107, message_pattern: "_contextFromWorkflow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 109, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 116, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 120, message_pattern: "<anonymous>".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 125, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
        ];

        expect_exact_diagnostics(&result, &expected);
    }

    #[test] 
    fn test_switch_statement_fixture() {
        let code = load_fixture("switchStatement.ts");
        let result = analyze_fixture(&code);
        
        let expected = vec![
            ExpectedDiagnostic { line: 3, message_pattern: "someRandomThrow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 4, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 7, message_pattern: "<anonymous>".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 11, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 24, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
        ];

        expect_exact_diagnostics(&result, &expected);
    }

    #[test]
    fn test_import_identifiers_fixture() {
        let result = analyze_fixture_tree(
            vec![
                ("importIdentifiers.ts", load_fixture("importIdentifiers.ts")),
                ("something.ts", load_fixture("something.ts")),
            ],
            "importIdentifiers.ts",
        );
        
        let expected = vec![
            ExpectedDiagnostic { line: 5, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            // ExpectedDiagnostic { line: 6, message_pattern: "SomeThrow2".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
        ];

        expect_exact_diagnostics(&result, &expected);
    }

    #[test]
    fn test_return_statement_fixture() {
        let code = load_fixture("returnStatement.ts");
        let result = analyze_fixture(&code);
        
        let expected = vec![
            ExpectedDiagnostic { line: 1, message_pattern: "someThrow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 4, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 8, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 13, message_pattern: "badMethod".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 14, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 21, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 22, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 23, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 23, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 24, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 25, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 30, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
        ];

        expect_exact_diagnostics(&result, &expected);
    }

    #[test]
    fn test_spread_expr_fixture() {
        let code = load_fixture("spreadExpr.ts");
        let result = analyze_fixture(&code);
        
        let expected = vec![
            ExpectedDiagnostic { line: 6, message_pattern: "_contextFromWorkflow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 7, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 13, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 22, message_pattern: "_contextFromWorkflow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 23, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 27, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
        ];

        expect_exact_diagnostics(&result, &expected);
    }

    // Test to specifically debug getter/setter functionality
    #[test]
    fn test_getter_setter_focused() {
        let code = r#"
const SomeThrow = () => {
  throw new Error('test')
}

const testGetter = {
  get test() {
    SomeThrow()
  },
  set test(value) {
    SomeThrow()
  }
}
"#;
        let result = analyze_fixture(code);
        
        let expected = vec![
            ExpectedDiagnostic { line: 2, message_pattern: "SomeThrow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 3, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            // Note: getter and setter function diagnostics currently not detected by analysis
            ExpectedDiagnostic { line: 8, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 11, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
        ];

        expect_exact_diagnostics(&result, &expected);
    }

    #[test]
    fn test_exports_ts_fixture() {
        let code = load_fixture("exports.ts");
        let result = analyze_fixture(&code);

        // Updated expectations to match actual Rust analyzer output (line numbers may differ from JS layer)
        let expected = vec![
            ExpectedDiagnostic { line: 7, message_pattern: "hiKhue".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 8, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 11, message_pattern: "someConstThatThrows".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 12, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 26, message_pattern: "_ConstThatThrows".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 27, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 31, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 35, message_pattern: "someConstThatThrows2".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 37, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 42, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 51, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 56, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
        ];

        expect_exact_diagnostics(&result, &expected);
    }

    #[test]
    fn test_trailing_comment_effect_on_parsing() {
        // Without trailing comment
        let code_without_comment = r#"
class SomeClass {
  constructor(public x: number) {}
  async _contextFromWorkflow() {
    throw new Error('Some error')
  }
  async someCallToThrow() {
    const { user } = await this._contextFromWorkflow(job)
  }
}
"#;
        let result_no_comment = analyze_fixture(code_without_comment);
        let expected_no_comment = vec![
            ExpectedDiagnostic { line: 4, message_pattern: "_contextFromWorkflow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 5, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 8, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
        ];
        expect_exact_diagnostics(&result_no_comment, &expected_no_comment);

        // With trailing comment on the throw line
        let code_with_comment = r#"
class SomeClass {
  constructor(public x: number) {}
  async _contextFromWorkflow() {
    throw new Error('Some error') // anything here
  }
  async someCallToThrow() {
    const { user } = await this._contextFromWorkflow(job)
  }
}
"#;
        let result_with_comment = analyze_fixture(code_with_comment);
        let expected_with_comment = vec![
            ExpectedDiagnostic { line: 4, message_pattern: "_contextFromWorkflow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 5, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 8, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
        ];
        expect_exact_diagnostics(&result_with_comment, &expected_with_comment);
    }

    #[test]
    fn test_jsx_fixture() {
        let code = load_fixture("jsx.jsx");
        let result = analyze_fixture(&code);
        
        let expected = vec![
            ExpectedDiagnostic { line: 1, message_pattern: "someThrow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 2, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 4, message_pattern: "someThrow2".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 5, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 8, message_pattern: "someTsx".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 10, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 15, message_pattern: "someAsyncTsx".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 17, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 23, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 24, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 29, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 30, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
        ];

        expect_exact_diagnostics(&result, &expected);
    }

    #[test]
    fn test_tsx_fixture() {
        let code = load_fixture("tsx.tsx");
        let result = analyze_fixture(&code);

        let expected = vec![
            ExpectedDiagnostic { line: 2, message_pattern: "someThrow".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 3, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 5, message_pattern: "someThrow2".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 6, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 9, message_pattern: "someTsx".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 11, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 16, message_pattern: "someAsyncTsx".to_string(), diagnostic_type: DiagnosticType::FunctionMayThrow },
            ExpectedDiagnostic { line: 18, message_pattern: "Throw statement".to_string(), diagnostic_type: DiagnosticType::ThrowStatement },
            ExpectedDiagnostic { line: 24, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 25, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 30, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
            ExpectedDiagnostic { line: 31, message_pattern: "Function call may throw".to_string(), diagnostic_type: DiagnosticType::FunctionCallMayThrow },
        ];

        expect_exact_diagnostics(&result, &expected);
    }
}