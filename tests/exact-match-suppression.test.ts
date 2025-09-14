import { describe, it, expect } from "vitest";
import { analyzeCode, expectExactDiagnostics, stripLineNumbers } from "./test-utils";

describe("Exact match @it-throws suppression", () => {
  it("should suppress function diagnostic with exact @it-throws comment", () => {
    const code = stripLineNumbers`
1 | // @it-throws
2 | function testFunction() {
3 |   throw new Error('test');
4 | }`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {});
  });

  it("should NOT suppress with partial @it-throws comment", () => {
    const code = stripLineNumbers`
1 | // this is an example of @it-throws comment
2 | function testFunction() {
3 |   throw new Error('test');
4 | }`;

    const diagnostics = analyzeCode(code);

    // Should have both function and throw statement diagnostics since comment doesn't match exactly
    expectExactDiagnostics(diagnostics, {
      L2: "testFunction may throw",
      L3: "Throw statement.",
    });
  });

  it("should NOT suppress with @it-throws in middle of comment", () => {
    const code = stripLineNumbers`
1 | // TODO: add @it-throws to this function
2 | function testFunction() {
3 |   throw new Error('test');
4 | }`;

    const diagnostics = analyzeCode(code);

    // Should have both function and throw statement diagnostics
    expectExactDiagnostics(diagnostics, {
      L2: "testFunction may throw",
      L3: "Throw statement.",
    });
  });

  it("should only suppress the specific function, not others", () => {
    const code = stripLineNumbers`
1 | // @it-throws
2 | function testFunction1() {
3 |   throw new Error('test');
4 | }
5 | 
6 | function testFunction2() {
7 |   throw new Error('test2');
8 | }`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      L6: "testFunction2 may throw",
      L7: "Throw statement.",
    });
  });

  it("should work with extra whitespace", () => {
    const code = stripLineNumbers`
1 | // @it-throws  
2 | function testFunction() {
3 |   throw new Error('test');
4 | }`;

    const diagnostics = analyzeCode(code);

    // Should only have throw statement diagnostic
    expectExactDiagnostics(diagnostics, {});
  });
});