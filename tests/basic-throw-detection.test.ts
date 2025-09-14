import { describe, it, expect } from "vitest";
import { analyzeCode, expectExactDiagnostics, stripLineNumbers } from "./test-utils";

describe("Basic throw detection", () => {
  it("should detect simple throw statements", () => {
    const code = stripLineNumbers`
1 | function simpleThrow() {
2 |   throw new Error("test error");
3 | }`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      L1: "simpleThrow may throw",
      L2: "Throw statement.",
    });
  });

  it("should detect multiple throws in a function", () => {
    const code = stripLineNumbers`
1 | function multipleThrows() {
2 |   if (Math.random() > 0.5) {
3 |     throw new Error("first error");
4 |   }
5 |   throw new TypeError("second error");
6 | }`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      L1: "multipleThrows may throw",
      L3: "Throw statement.",
      L5: "Throw statement.",
    });
  });

  it("should detect throws in arrow functions", () => {
    const code = stripLineNumbers`
1 | const arrowThrow = () => {
2 |   throw new Error("arrow error");
3 | };
4 | 
5 | const anotherArrow = () => {
6 |   throw new TypeError("another error");  
7 | };`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      L1: "arrowThrow may throw",
      L2: "Throw statement.",
      L5: "anotherArrow may throw",
      L6: "Throw statement.",
    });
  });

  it("inner it throws should suppress outer it throws", () => {
    const code = stripLineNumbers`
1 | function testFunction() {
2 |   // @it-throws
3 |   throw new Error('test');
4 | }`;

    const diagnostics = analyzeCode(code);
    expectExactDiagnostics(diagnostics, {});
  });

  it("outer it throws should suppress all diagnostics", () => {
    const code = stripLineNumbers`
1 | // @it-throws
2 | function testFunction() {
3 |   throw new Error('test');
4 | }`;

    const diagnostics = analyzeCode(code);
    expectExactDiagnostics(diagnostics, {});
  });

  it("first it throws should only suppress first function", () => {
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

  it("should detect function calls that may throw", () => {
    const code = stripLineNumbers`
1 | function throwsError() {
2 |   throw new Error("helper error");
3 | }
4 | 
5 | function callsThrowingFunction() {
6 |   throwsError(); // This call should be detected
7 |   return "done";
8 | }`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      "L1": "Function throwsError may throw: {Error}",
      "L2": "Throw statement.",
      "L5": [
        "Function callsThrowingFunction may throw: {Error}",
        "Throw statement."
      ],
      "L6": "Function call may throw: {Error}."
    });
  });

  it.skip("should handle nested function calls", () => {
    const code = stripLineNumbers`
1 | function level1() {
2 |   throw new Error("level 1");
3 | }
4 | 
5 | function level2() {
6 |   level1();
7 | }
8 | 
9 | function level3() {
10|   level2();
11| }`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      L1: "Function level1 may throw: {Error}",
      L2: "Throw statement.",
      L5: "level2 may throw {Error} when level 1 is called",
      L6: "level1 may call may throw: {Error}",
      L9: "level3 may throw {Error} when level 2 is called",
      L10: "level2 may call may throw: {Error}",
    });
  });

  it("should detect different error types", () => {
    const code = stripLineNumbers`
1 | function throwsBuiltIn() {
2 |   throw new TypeError("type error");
3 | }
4 | 
5 | function throwsCustom() {
6 |   throw new CustomError("custom error");
7 | }
8 | 
9 | class CustomError extends Error {}`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      L1: "throwsBuiltIn may throw",
      L2: "Throw statement.",
      L5: "throwsCustom may throw",
      L6: "Throw statement.",
    });
  });
});
