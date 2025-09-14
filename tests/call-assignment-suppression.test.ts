import { describe, it, expect } from 'vitest';
import { analyzeCode, expectExactDiagnostics, stripLineNumbers } from './test-utils';

describe('@it-throws call assignment suppression', () => {
  it('should suppress direct function calls with @it-throws comments', () => {
    const code = stripLineNumbers`
1 | function throwsError() {
2 |   throw new Error("test");
3 | }
4 | 
5 | function testFunction() {
6 |   // @it-throws
7 |   throwsError(); // Should be suppressed
8 | 
9 |   throwsError(); // Should NOT be suppressed (no comment)
10| }`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      "L1": "Function throwsError may throw: {Error}",
      "L2": "Throw statement.",
      "L5": [
        "Function testFunction may throw: {Error}",
        "Throw statement."
      ],
      "L9": "Function call may throw: {Error}."
    });
  });

  it('should suppress assignment calls with @it-throws comments', () => {
    const code = stripLineNumbers`
1 | function returnsValue() {
2 |   if (Math.random() > 0.5) throw new Error("random error");
3 |   return "value";
4 | }
5 | 
6 | function testAssignments() {
7 |   // @it-throws
8 |   const result1 = returnsValue(); // Should be suppressed
9 | 
10|   // @it-throws  
11|   let result2 = returnsValue(); // Should be suppressed
12| 
13|   const result3 = returnsValue(); // Should NOT be suppressed (no comment)
14| }`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      "L1": "Function returnsValue may throw: {Error}",
      "L2": "Throw statement.",
      "L6": [
        "Function testAssignments may throw: {Error}",
        "Throw statement."
      ],
      "L13": "Function call may throw: {Error}."
    });
  });

  it('should handle complex assignment patterns', () => {
    const code = stripLineNumbers`
1 | function getData() {
2 |   throw new Error("data error");
3 | }
4 | 
5 | function processData() {
6 |   // @it-throws
7 |   const { data, error } = getData(); // Destructuring assignment
8 | 
9 |   // @it-throws
10|   const [first, second] = getData(); // Array destructuring
11| 
12|   // @it-throws
13|   const computed = getData().property; // Property access
14| 
15|   const normal = getData(); // Should NOT be suppressed
16| }`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      "L1": "Function getData may throw: {Error}",
      "L2": "Throw statement.",
      "L5": [
        "Function processData may throw: {Error}",
        "Throw statement."
      ],
      "L15": "Function call may throw: {Error}."
    });
  });

  it('should work with nested function calls in assignments', () => {
    const code = stripLineNumbers`
1 | function outer() {
2 |   throw new Error("outer");
3 | }
4 | 
5 | function inner() {
6 |   throw new Error("inner");
7 | }
8 | 
9 | function testNested() {
10|   // @it-throws
11|   const result = outer(inner()); // Should suppress the outer call
12| 
13|   const result2 = outer(inner()); // Should NOT suppress anything
14| }`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
        "L1": "Function outer may throw: {Error}",
        "L2": "Throw statement.",
        "L5": "Function inner may throw: {Error}",
        "L6": "Throw statement.",
        "L9": [
          "Function testNested may throw: {Error}",
          "Throw statement."
        ],
        "L13": "Function call may throw: {Error}."
    });
  });

});