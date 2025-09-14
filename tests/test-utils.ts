// @it-throws-disable
import { InputData, parse_js } from '../server/src/rust/what_does_it_throw_wasm.js';
import { computeQuickFixesForDiagnostics, Range } from '../server/src/codeActions';
import { readFileSync } from 'fs';
import { join } from 'path';

export interface TestDiagnostic {
  line: number;
  message: string;
  severity: number;
}


export function analyzeCode(code: string, options: Partial<InputData> & { uri?: string } = {}): TestDiagnostic[] {
  const defaultOptions = {
    debug: false,
    throw_statement_severity: "Warning",
    function_throw_severity: "Warning",
    call_to_throw_severity: "Warning",
    call_to_imported_throw_severity: "Warning",
    include_try_statement_throws: false,
    ignore_statements: ["@it-throws"],
    uri: options.uri ?? "test.ts",
    file_content: code ?? "",
    ...options
  } as InputData;

  const result = parse_js(defaultOptions);

  if (!result.diagnostics) {
    return [];
  }

  return result.diagnostics.map((diagnostic: any) => ({
    line: diagnostic.range.start.line, // Now 1-based from Rust
    message: diagnostic.message,
    severity: diagnostic.severity
  }));
}

export interface TestQuickFixEdit {
  startLine: number
  startCharacter: number
  endLine: number
  endCharacter: number
  newText: string
}

export interface TestQuickFix {
  title: string
  edits: TestQuickFixEdit[]
}

export function computeQuickFixes(code: string, diagnosticsRaw: any[]): TestQuickFix[] {
  // diagnosticsRaw here is the raw result.diagnostics from WASM (1-based lines)
  const diagnostics = diagnosticsRaw.map((d: any) => ({
    message: d.message,
    source: d.source,
    range: {
      start: { line: Math.max(0, d.range.start.line - 1), character: d.range.start.character },
      end: { line: Math.max(0, d.range.end.line - 1), character: d.range.end.character }
    } satisfies Range
  }))

  const actions = computeQuickFixesForDiagnostics(code, diagnostics)
  return actions.map(a => ({
    title: a.title,
    edits: a.edits.map(e => ({
      startLine: e.range.start.line + 1,
      startCharacter: e.range.start.character,
      endLine: e.range.end.line + 1,
      endCharacter: e.range.end.character,
      newText: e.newText
    }))
  }))
}

export function expectExactQuickFixes(
  code: string,
  diagnosticsRaw: any[],
  expected: { [key: string]: string[] }
) {
  // expected: { title: ["snippet1", "snippet2" ] }
  const fixes = computeQuickFixes(code, diagnosticsRaw)
  const actualTitles = fixes.map(f => f.title).sort()
  const expectedTitles = Object.keys(expected).sort()
  if (actualTitles.join('\n') !== expectedTitles.join('\n')) {
    // eslint-disable-next-line no-console
    console.log(blue('\nActual quick fix titles:\n' + JSON.stringify(actualTitles, null, 2)))
    // eslint-disable-next-line no-console
    console.log('\nDiff (green = actual only, red = expected only, grey = same):\n' + colorDiff(JSON.stringify(actualTitles, null, 2), JSON.stringify(expectedTitles, null, 2)))
    throw new Error(`Quick fix titles mismatch.`)
  }

  for (const [title, snippets] of Object.entries(expected)) {
    const fix = fixes.find(f => f.title === title)
    if (!fix) {
      throw new Error(`Missing quick fix '${title}'`)
    }
    const appliedText = fix.edits.map(e => e.newText).join('\n')
    for (const snippet of snippets) {
      if (!appliedText.includes(snippet)) {
        // eslint-disable-next-line no-console
        console.log(blue('\nActual fix text for ' + title + ':\n' + appliedText))
        throw new Error(`Expected snippet not found in quick fix '${title}': ${snippet}`)
      }
    }
  }
}

/**
 * Template tag function that strips line numbers from code strings.
 * Allows you to write test code with line numbers for easier debugging:
 * 
 * const code = stripLineNumbers`
 * 1 | function test() {
 * 2 |   throw new Error();
 * 3 | }`;
 * 
 * The function will:
 * 1. Remove the first line if it's empty
 * 2. Strip line numbers and the `|` separator from each line
 * 3. Return clean code string
 */
export function stripLineNumbers(strings: TemplateStringsArray, ...values: any[]): string {
  // Combine template strings and values
  let result = strings[0];
  for (let i = 0; i < values.length; i++) {
    result += values[i] + strings[i + 1];
  }
  
  // Split into lines
  const lines = result.split('\n');
  
  // Remove first line if it's empty (common when starting template on new line)
  if (lines.length > 0 && lines[0].trim() === '') {
    lines.shift();
  }
  
  // Process each line to remove line numbers and separator
  const processedLines = lines.map(line => {
    // Match pattern: optional whitespace, digits, optional whitespace, |, then the rest
    const match = line.match(/^\s*\d+\s*\|\s?(.*)$/);
    if (match) {
      return match[1]; // Return everything after the | separator
    }
    return line; // Return original line if no number pattern found
  });
  
  return processedLines.join('\n');
}

/**
 * Load a fixture file from the Rust crate fixtures directory.
 * 
 * @param fixtureName - Name of the fixture file (e.g., 'ignoreStatements.ts')
 * @returns The content of the fixture file as a string
 */
export function loadFixture(fixtureName: string): string {
  const fixturePath = join(__dirname, '../crates/what-does-it-throw/src/fixtures', fixtureName);
  return readFileSync(fixturePath, 'utf-8');
}

/**
 * Assert that diagnostics match exactly the expected map and no other diagnostics exist.
 * 
 * @param diagnostics - The actual diagnostics from the analyzer
 * @param expectedMap - Map of line numbers to expected message patterns (can be string, RegExp, or array for multiple diagnostics per line)
 * 
 * @example
 * expectExactDiagnostics(diagnostics, {
 *   'L21': 'someMethodThatThrows2 may throw',
 *   'L24': 'Throw statement.',
 *   'L89': 'Function call may throw'
 * });
 * 
 * // For multiple diagnostics on the same line:
 * expectExactDiagnostics(diagnostics, {
 *   'L1': ['function may throw', 'Throw statement.']
 * });
 */
import { diffLines } from 'diff';

function color(str: string, colorCode: string) {
  return `\x1b[${colorCode}m${str}\x1b[0m`;
}
function blue(str: string) {
  return color(str, '34');
}
function green(str: string) {
  return color(str, '32');
}
function red(str: string) {
  return color(str, '31');
}
function grey(str: string) {
  return color(str, '90');
}

function colorDiff(actual: string, expected: string) {
  // Use diffLines for multi-line diff
  const diff = diffLines(expected, actual);
  let out = '';
  for (const part of diff) {
    if (part.added) {
      out += green(part.value);
    } else if (part.removed) {
      out += red(part.value);
    } else {
      out += grey(part.value);
    }
  }
  return out;
}

function buildLineToMessagesMap(
  diagnostics: TestDiagnostic[]
): { [key: `L${number}`]: string | string[] } {
  const grouped: Record<string, string[]> = {};
  for (const d of diagnostics) {
    const key = `L${d.line}` as const;
    if (!grouped[key]) grouped[key] = [];
    grouped[key].push(d.message);
  }

  const orderedEntries = Object.keys(grouped)
    .sort((a, b) => parseInt(a.slice(1), 10) - parseInt(b.slice(1), 10))
    .map((k) => {
      const msgs = grouped[k];
      if (msgs.length === 1) return [k, msgs[0]] as const;
      return [k, msgs] as const;
    });

  return Object.fromEntries(orderedEntries) as { [key: `L${number}`]: string | string[] };
}

export function expectExactDiagnostics(
  diagnostics: TestDiagnostic[], 
  expectedMap: { [key: `L${number}`]: string | RegExp | (string | RegExp)[] }
) {
  // Calculate total expected diagnostics (counting arrays)
  let totalExpectedDiagnostics = 0;
  const expectedLines: number[] = [];
  
  for (const [key, patterns] of Object.entries(expectedMap)) {
    const match = key.match(/^L(\d+)$/);
    if (!match) {
      throw new Error(`Invalid key format: ${key}. Expected format: L<number> (e.g., L21)`);
    }
    const line = parseInt(match[1], 10);
    expectedLines.push(line);
    
    if (Array.isArray(patterns)) {
      totalExpectedDiagnostics += patterns.length;
    } else {
      totalExpectedDiagnostics += 1;
    }
  }

  expectedLines.sort((a, b) => a - b);

  // Get actual line numbers and count from diagnostics
  const actualLines = diagnostics.map(d => d.line).sort((a, b) => a - b);
  const totalActualDiagnostics = diagnostics.length;

  // Check if total diagnostic count matches
  if (totalExpectedDiagnostics !== totalActualDiagnostics) {
    const actualObj = buildLineToMessagesMap(diagnostics);
    const actualStr = JSON.stringify(actualObj, null, 2);
    const expectedObj: { [key: `L${number}`]: string | string[] } = {} as any;
    for (const [key, patterns] of Object.entries(expectedMap)) {
      if (Array.isArray(patterns)) {
        expectedObj[key as `L${number}`] = patterns.map(p => typeof p === 'string' ? p : p.toString());
      } else {
        expectedObj[key as `L${number}`] = typeof patterns === 'string' ? patterns : patterns.toString();
      }
    }
    const expectedStr = JSON.stringify(expectedObj, null, 2);
    // eslint-disable-next-line no-console
    console.log(blue('\nActual diagnostics (by line):\n' + '\n' + actualStr));
    // eslint-disable-next-line no-console
    console.log('\nDiff (green = actual only, red = expected only, grey = same):\n' + colorDiff(actualStr, expectedStr));
    throw new Error(
      `Expected ${totalExpectedDiagnostics} diagnostics, ` +
      `but got ${totalActualDiagnostics} diagnostics.`
    );
  }

  // Verify message patterns for each expected diagnostic
  for (const [key, expectedPatterns] of Object.entries(expectedMap)) {
    const lineMatch = key.match(/^L(\d+)$/);
    if (!lineMatch) continue; // Already validated above
    
    const line = parseInt(lineMatch[1], 10);
    const lineDiagnostics = diagnostics.filter(d => d.line === line);
    
    if (lineDiagnostics.length === 0) {
      const actualObj = buildLineToMessagesMap(diagnostics);
      const actualStr = JSON.stringify(actualObj, null, 2);
      const expectedObj: { [key: `L${number}`]: string | string[] } = {} as any;
      for (const [k, patterns] of Object.entries(expectedMap)) {
        if (Array.isArray(patterns)) {
          expectedObj[k as `L${number}`] = patterns.map(p => typeof p === 'string' ? p : p.toString());
        } else {
          expectedObj[k as `L${number}`] = typeof patterns === 'string' ? patterns : patterns.toString();
        }
      }
      const expectedStr = JSON.stringify(expectedObj, null, 2);
      // eslint-disable-next-line no-console
      console.log(blue('\nActual diagnostics (by line):\n' + actualStr));
      // eslint-disable-next-line no-console
      console.log('\nDiff (green = actual only, red = expected only, grey = same):\n' + colorDiff(actualStr, expectedStr));
      throw new Error(`No diagnostics found at line ${line}`);
    }

    // Handle array of patterns (multiple diagnostics on same line)
    if (Array.isArray(expectedPatterns)) {
      if (lineDiagnostics.length !== expectedPatterns.length) {
        const actualObj = buildLineToMessagesMap(diagnostics);
        const actualStr = JSON.stringify(actualObj, null, 2);
        const expectedObj: { [key: `L${number}`]: string | string[] } = {} as any;
        for (const [k, pats] of Object.entries(expectedMap)) {
          if (Array.isArray(pats)) {
            expectedObj[k as `L${number}`] = pats.map(p => typeof p === 'string' ? p : p.toString());
          } else {
            expectedObj[k as `L${number}`] = typeof pats === 'string' ? pats : pats.toString();
          }
        }
        const expectedStr = JSON.stringify(expectedObj, null, 2);
        // eslint-disable-next-line no-console
        console.log(blue('\nActual diagnostics (by line):\n' + actualStr));
        // eslint-disable-next-line no-console
        console.log('\nDiff (green = actual only, red = expected only, grey = same):\n' + colorDiff(actualStr, expectedStr));
        throw new Error(
          `Expected ${expectedPatterns.length} diagnostics at line ${line}, ` +
          `but got ${lineDiagnostics.length}.`
        );
      }
      
      // Check that each expected pattern matches at least one diagnostic on this line
      for (const expectedPattern of expectedPatterns) {
        const matchFound = lineDiagnostics.some(diagnostic => {
          if (typeof expectedPattern === 'string') {
            return diagnostic.message.includes(expectedPattern);
          } else {
            return expectedPattern.test(diagnostic.message);
          }
        });
        
        if (!matchFound) {
          const actualObj = buildLineToMessagesMap(diagnostics);
          const actualStr = JSON.stringify(actualObj, null, 2);
          const expectedObj: { [key: `L${number}`]: string | string[] } = {} as any;
          for (const [k, pats] of Object.entries(expectedMap)) {
            if (Array.isArray(pats)) {
              expectedObj[k as `L${number}`] = pats.map(p => typeof p === 'string' ? p : p.toString());
            } else {
              expectedObj[k as `L${number}`] = typeof pats === 'string' ? pats : pats.toString();
            }
          }
          const expectedStr = JSON.stringify(expectedObj, null, 2);
          // eslint-disable-next-line no-console
          console.log(blue('\nActual diagnostics (by line):\n' + actualStr));
          // eslint-disable-next-line no-console
          console.log('\nDiff (green = actual only, red = expected only, grey = same):\n' + colorDiff(actualStr, expectedStr));
          throw new Error(
            `No diagnostic at line ${line} matches expected pattern: ${expectedPattern}`
          );
        }
      }
    } else {
      // Single pattern
      if (lineDiagnostics.length !== 1) {
        const actualObj = buildLineToMessagesMap(diagnostics);
        const actualStr = JSON.stringify(actualObj, null, 2);
        const expectedObj: { [key: `L${number}`]: string | string[] } = {} as any;
        for (const [k, pats] of Object.entries(expectedMap)) {
          if (Array.isArray(pats)) {
            expectedObj[k as `L${number}`] = pats.map(p => typeof p === 'string' ? p : p.toString());
          } else {
            expectedObj[k as `L${number}`] = typeof pats === 'string' ? pats : pats.toString();
          }
        }
        const expectedStr = JSON.stringify(expectedObj, null, 2);
        // eslint-disable-next-line no-console
        console.log(blue('\nActual diagnostics (by line):\n' + actualStr));
        // eslint-disable-next-line no-console
        console.log('\nDiff (green = actual only, red = expected only, grey = same):\n' + colorDiff(actualStr, expectedStr));
        throw new Error(
          `Expected 1 diagnostic at line ${line}, but got ${lineDiagnostics.length}.`
        );
      }
      
      const diagnostic = lineDiagnostics[0];
      if (typeof expectedPatterns === 'string') {
        if (!diagnostic.message.includes(expectedPatterns)) {
          const actualObj = buildLineToMessagesMap(diagnostics);
          const actualStr = JSON.stringify(actualObj, null, 2);
          const expectedObj: { [key: `L${number}`]: string | string[] } = {} as any;
          for (const [k, pats] of Object.entries(expectedMap)) {
            if (Array.isArray(pats)) {
              expectedObj[k as `L${number}`] = pats.map(p => typeof p === 'string' ? p : p.toString());
            } else {
              expectedObj[k as `L${number}`] = typeof pats === 'string' ? pats : pats.toString();
            }
          }
          const expectedStr = JSON.stringify(expectedObj, null, 2);
          // eslint-disable-next-line no-console
          console.log(blue('\nActual diagnostics (by line):\n' + actualStr));
          // eslint-disable-next-line no-console
          console.log('\nDiff (green = actual only, red = expected only, grey = same):\n' + colorDiff(actualStr, expectedStr));
          throw new Error(
            `Diagnostic at line ${line} does not contain expected pattern.\n` +
            `Expected: "${expectedPatterns}"\n` +
            `Actual: "${diagnostic.message}"`
          );
        }
      } else {
        if (!expectedPatterns.test(diagnostic.message)) {
          const actualObj = buildLineToMessagesMap(diagnostics);
          const actualStr = JSON.stringify(actualObj, null, 2);
          const expectedObj: { [key: `L${number}`]: string | string[] } = {} as any;
          for (const [k, pats] of Object.entries(expectedMap)) {
            if (Array.isArray(pats)) {
              expectedObj[k as `L${number}`] = pats.map(p => typeof p === 'string' ? p : p.toString());
            } else {
              expectedObj[k as `L${number}`] = typeof pats === 'string' ? pats : pats.toString();
            }
          }
          const expectedStr = JSON.stringify(expectedObj, null, 2);
          // eslint-disable-next-line no-console
          console.log(blue('\nActual diagnostics (by line):\n' + actualStr));
          // eslint-disable-next-line no-console
          console.log('\nDiff (green = actual only, red = expected only, grey = same):\n' + colorDiff(actualStr, expectedStr));
          throw new Error(
            `Diagnostic at line ${line} does not match expected pattern.\n` +
            `Expected pattern: ${expectedPatterns}\n` +
            `Actual: "${diagnostic.message}"`
          );
        }
      }
    }
  }
}