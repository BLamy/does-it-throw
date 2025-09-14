import { describe, it } from 'vitest'
import { analyzeCode, expectExactDiagnostics, expectExactQuickFixes, stripLineNumbers } from './test-utils'
import { parse_js, InputData } from '../server/src/rust/what_does_it_throw_wasm.js'

function buildParseOptions(code: string) {
  return {
    file_content: code,
    debug: false,
    throw_statement_severity: 'Hint',
    function_throw_severity: 'Hint',
    call_to_throw_severity: 'Hint',
    call_to_imported_throw_severity: 'Hint',
    include_try_statement_throws: false,
    ignore_statements: ['@it-throws']
  } satisfies InputData
}

describe('Callback parameter @throws annotation behavior', () => {
  const header = stripLineNumbers`
1 | export class TypedError extends Error {
2 |   constructor(message: string) {
3 |     super(message);
4 |     this.name = 'TypedError';
5 |   }
6 | }
7 |
8 | const SomeRandomCall = (fn: () => void /** @throws {TypedError} */) => {
9 |   fn()
10| }
`

  it('accepts a callback that throws the documented TypedError', () => {
    const code = header + stripLineNumbers`
11| SomeRandomCall(() => { 
12|   throw new TypedError('hi khue')
13| })
`

    const diagnostics = analyzeCode(code)

    // Desired behavior: no diagnostics (documented error type for parameter)
    expectExactDiagnostics(diagnostics, {})

    const result = parse_js(buildParseOptions(code)) as any
    // No quick fixes expected when there are no diagnostics
    expectExactQuickFixes(code, result.diagnostics, {})
  })

  it('flags a callback that throws a different error type', () => {
    const code = header + stripLineNumbers`
11| SomeRandomCall(() => { // should be flagged can only take a function which throws a TypedError
12|   throw new Error('hi khue')
13| })
`

    const diagnostics = analyzeCode(code)

    // Expect: anonymous function may throw + throw statement
    expectExactDiagnostics(diagnostics, {
      'L11': 'Anonymous function may throw',
      'L12': 'Throw statement.'
    })

    const result = parse_js(buildParseOptions(code) as any) as any
    // Expect quick fixes for anonymous callback
    expectExactQuickFixes(code, result.diagnostics, {
      'Annotate anonymous function with @throws': ['@throws {Error}'],
      'Convert to named function with @throws': ['function callbackThrows(']
    })
  })

  it.skip('allows callback when suppressed by @it-throws inside the callback', () => {
    const code = header + stripLineNumbers`
11| SomeRandomCall(() => {
12|   // @it-throws
13|   throw new Error('hi khue')
14| })
`

    const diagnostics = analyzeCode(code)
    expectExactDiagnostics(diagnostics, {})

    const result = parse_js(buildParseOptions(code) as any) as any
    expectExactQuickFixes(code, result.diagnostics, {})
  })
})


