# Test Suite

This directory contains the Vitest test suite for the "Does it Throw?" extension.

## Running Tests

```bash
# Run all tests
bun run test

# Run tests in watch mode
bun run test:watch

# Run tests with UI
bun run test:ui

# Run tests once (CI mode)
bun run test:run
```

## Test Structure

### `test-utils.ts`
Helper utilities for testing the WASM module:
- `analyzeCode()` - Analyze code and return diagnostics
- `expectDiagnostic()` - Assert a diagnostic exists at a specific line
- `expectNoDiagnosticAtLine()` - Assert no diagnostic at a line
- `expectNoDiagnosticsContaining()` - Assert no diagnostics match a pattern

### `comprehensive-suppression.test.ts` ✅
Tests the main feature - comprehensive @it-throws suppression:
- Function-level suppression
- Call-level suppression  
- Original user scenario validation

### `basic-throw-detection.test.ts` (needs line number fixes)
Basic functionality tests:
- Simple throw detection
- Function call detection
- Nested calls

### `call-assignment-suppression.test.ts` (needs fixes)
Tests for the @it-throws comment detection in assignments:
- Direct function calls with @it-throws
- Assignment calls with @it-throws
- Comment distance validation

### `it-throws-suppression.test.ts` (needs line number fixes)
Extended suppression tests:
- Multiple scenarios
- Edge cases
- Arrow functions

## Current Status

✅ **Working Tests**: 
- `comprehensive-suppression.test.ts` - Core functionality tests pass

⚠️ **Needs Fixes**:
- Other test files have line number alignment issues
- Some edge cases need investigation

## Key Achievement

The comprehensive @it-throws suppression is **working perfectly**:

```typescript
// @it-throws  
function myFunction() {
  throwingCall();     // ✅ Suppressed
  throw new Error();  // ✅ Suppressed
  // Function diagnostic ✅ Suppressed
}
```

This solves the original user issue where @it-throws comments should suppress ALL diagnostics for a function, not just individual calls.