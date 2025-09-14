import { resolve, join } from 'path';
import { existsSync } from 'fs';

// Wallaby-specific test utilities that handle WASM loading more robustly

export interface TestDiagnostic {
  line: number;
  message: string;
  severity: number;
}

export interface ParseOptions {
  debug?: boolean;
  throw_statement_severity?: string;
  function_throw_severity?: string;
  call_to_throw_severity?: string;
  call_to_imported_throw_severity?: string;
  include_try_statement_throws?: boolean;
  ignore_statements?: string[];
}

// Lazy-loaded WASM module to handle different environments
let wasmModule: any = null;

function loadWasmModule() {
  if (wasmModule) {
    return wasmModule;
  }

  // Try different paths for the WASM module
  const possiblePaths = [
    resolve(process.cwd(), 'server/src/rust/what_does_it_throw_wasm.js'),
    resolve(__dirname, '../server/src/rust/what_does_it_throw_wasm.js'),
    './server/src/rust/what_does_it_throw_wasm.js',
    '../server/src/rust/what_does_it_throw_wasm.js'
  ];

  for (const path of possiblePaths) {
    try {
      if (existsSync(path)) {
        console.log(`Loading WASM module from: ${path}`);
        wasmModule = require(path);
        return wasmModule;
      }
    } catch (error) {
      console.warn(`Failed to load WASM module from ${path}:`, error.message);
    }
  }

  throw new Error(`Could not load WASM module from any of these paths: ${possiblePaths.join(', ')}`);
}

export function analyzeCode(code: string, options: ParseOptions = {}): TestDiagnostic[] {
  const defaultOptions: ParseOptions = {
    debug: false,
    throw_statement_severity: "Warning",
    function_throw_severity: "Warning",
    call_to_throw_severity: "Warning",
    call_to_imported_throw_severity: "Warning",
    include_try_statement_throws: false,
    ignore_statements: ["@it-throws"],
    ...options
  };

  try {
    const { parse_js } = loadWasmModule();
    
    const result = parse_js({
      file_content: code,
      ...defaultOptions
    });

    if (!result.diagnostics) {
      return [];
    }

    return result.diagnostics.map((diagnostic: any) => ({
      line: diagnostic.range.start.line + 1, // Convert to 1-based line numbers
      message: diagnostic.message,
      severity: diagnostic.severity
    }));
  } catch (error) {
    console.error('Error analyzing code:', error);
    throw error;
  }
}

export function expectDiagnostic(diagnostics: TestDiagnostic[], line: number, messagePattern: string | RegExp) {
  const diagnostic = diagnostics.find(d => d.line === line);
  if (!diagnostic) {
    throw new Error(`Expected diagnostic at line ${line}, but found none. Available diagnostics: ${JSON.stringify(diagnostics, null, 2)}`);
  }
  
  if (typeof messagePattern === 'string') {
    if (!diagnostic.message.includes(messagePattern)) {
      throw new Error(`Expected diagnostic at line ${line} to contain "${messagePattern}", but got: "${diagnostic.message}"`);
    }
  } else {
    if (!messagePattern.test(diagnostic.message)) {
      throw new Error(`Expected diagnostic at line ${line} to match pattern ${messagePattern}, but got: "${diagnostic.message}"`);
    }
  }
  
  return diagnostic;
}

export function expectNoDiagnosticAtLine(diagnostics: TestDiagnostic[], line: number) {
  const diagnostic = diagnostics.find(d => d.line === line);
  if (diagnostic) {
    throw new Error(`Expected no diagnostic at line ${line}, but found: "${diagnostic.message}"`);
  }
}

export function expectNoDiagnosticsContaining(diagnostics: TestDiagnostic[], pattern: string | RegExp) {
  const matchingDiagnostics = diagnostics.filter(d => {
    if (typeof pattern === 'string') {
      return d.message.includes(pattern);
    } else {
      return pattern.test(d.message);
    }
  });
  
  if (matchingDiagnostics.length > 0) {
    throw new Error(`Expected no diagnostics matching pattern ${pattern}, but found: ${JSON.stringify(matchingDiagnostics, null, 2)}`);
  }
}