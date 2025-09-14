import { describe, it, expect } from "vitest";
import { analyzeCode, expectExactDiagnostics, stripLineNumbers } from "./test-utils";

describe("File-level @it-throws-disable", () => {
  it("should disable all diagnostics with // @it-throws-disable at top of file", () => {
    const code = stripLineNumbers`
1 | // @it-throws-disable
2 | function throwsError() {
3 |   throw new Error("test error");
4 | }
5 |
6 | function callsThrowingFunction() {
7 |   throwsError();
8 | }`;

    const diagnostics = analyzeCode(code);

    // Should have no diagnostics at all
    expectExactDiagnostics(diagnostics, {})
  });

  it("should disable all diagnostics with /* @it-throws-disable */ at top of file", () => {
    const code = stripLineNumbers`
1 | /* @it-throws-disable */
2 | function throwsError() {
3 |   throw new Error("test error");
4 | }
5 |
6 | function callsThrowingFunction() {
7 |   throwsError();
8 | }`;

    const diagnostics = analyzeCode(code);

    // Should have no diagnostics at all
    expectExactDiagnostics(diagnostics, {})
  });

  it("should work when @it-throws-disable is not on the first line but within first 10 lines", () => {
    const code = stripLineNumbers`
1 | /**
2 |  * Test utility functions
3 |  * @author Test
4 |  */
5 | // @it-throws-disable
6 | 
7 | function throwsError() {
8 |   throw new Error("test error");
9 | }`;

    const diagnostics = analyzeCode(code);

    // Should have no diagnostics
    expectExactDiagnostics(diagnostics, {})
  });

  it("should NOT disable when @it-throws-disable is not exact match", () => {
    const code = stripLineNumbers`
1 | // TODO: add @it-throws-disable to this file
2 | function throwsError() {
3 |   throw new Error("test error");
4 | }`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      L2: "Function throwsError may throw: {Error}",
      L3: "Throw statement."
    })
  });

  it("should NOT disable when @it-throws-disable is too far down in file", () => {
    const code = stripLineNumbers`
1 | function throwsError() {
2 |   throw new Error("test error");
3 | }
4 |
5 | function another() {
6 |   return 1;
7 | }
8 |
9 | function yetAnother() {
10|   return 2;
11| }
12|
13| function andAnother() {
14|   return 3;
15| }
16|
17| // @it-throws-disable
18| function lastFunction() {
19|   throw new Error("too late");
20| }`;

    const diagnostics = analyzeCode(code);
    expectExactDiagnostics(diagnostics, {
      L1: "Function throwsError may throw: {Error}",
      L2: "Throw statement.",
      L18: "Function lastFunction may throw: {Error}",
      L19: "Throw statement."
    })
  });

  it("should work with mixed content before the disable comment", () => {
    const code = stripLineNumbers`
1 | #!/usr/bin/env node
2 | 
3 | /**
4 |  * @file Test utilities for throw detection
5 |  */
6 | 
7 | // @it-throws-disable
8 | 
9 | import { something } from './other';
10|
11| export function utilityFunction() {
12|   throw new Error("utility error");
13| }
14|
15| export function anotherUtility() {
16|   utilityFunction();
17|   throw new TypeError("another error");
18| }`;

    const diagnostics = analyzeCode(code);
    expectExactDiagnostics(diagnostics, {})
  });
});