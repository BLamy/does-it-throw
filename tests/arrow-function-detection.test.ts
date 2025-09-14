import { describe, it, expect } from "vitest";
import { analyzeCode, expectExactDiagnostics, stripLineNumbers } from "./test-utils";

describe("Arrow function throw detection", () => {
  it("should detect throws in arrow functions", () => {
    const code = stripLineNumbers`
1 | const arrowThrow = () => {
2 |   throw new Error("arrow error");
3 | };`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      'L1': 'arrowThrow may throw',
      'L2': 'Throw statement.',
    });
  });

  it("should compare arrow functions with normal functions", () => {
    const normalFunctionCode = stripLineNumbers`
1 | function normalThrow() {
2 |   throw new Error("normal error");  
3 | }`;

    const arrowFunctionCode = stripLineNumbers`
1 | const arrowThrow = () => {
2 |   throw new Error("arrow error");
3 | };`;

    const normalDiagnostics = analyzeCode(normalFunctionCode);
    const arrowDiagnostics = analyzeCode(arrowFunctionCode);

    // Both should have same structure: function diagnostic + throw diagnostic
    expectExactDiagnostics(normalDiagnostics, {
      'L1': 'normalThrow may throw',
      'L2': 'Throw statement.',
    });

    expectExactDiagnostics(arrowDiagnostics, {
      'L1': 'arrowThrow may throw',
      'L2': 'Throw statement.',
    });
  });

  it("should detect multiple arrow functions", () => {
    const code = stripLineNumbers`
1 | const first = () => {
2 |   throw new Error("first");
3 | };
4 |
5 | const second = () => {
6 |   throw new TypeError("second");
7 | };`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      'L1': 'first may throw',
      'L2': 'Throw statement.',
      'L5': 'second may throw',
      'L6': 'Throw statement.',
    });
  });
});