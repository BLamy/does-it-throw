import { describe, it, expect } from 'vitest';
import { analyzeCode, expectExactDiagnostics, stripLineNumbers } from './test-utils';

describe('Comprehensive @it-throws suppression', () => {
  it('should suppress all diagnostics for functions with @it-throws comment', () => {
    const code = stripLineNumbers`
1 | function throwsError() {
2 |   throw new Error("test");
3 | }
4 | 
5 | // @it-throws
6 | function withSuppression() {
7 |   throwsError(); // Should be suppressed
8 |   throw new Error("also suppressed"); // Should be suppressed
9 | }
10| 
11| function withoutSuppression() {
12|   throwsError(); // Should NOT be suppressed  
13|   throw new Error("also not suppressed"); // Should NOT be suppressed
14| }`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      L1: "throwsError may throw",
      L2: "Throw statement.",
      L11: "withoutSuppression may throw",
      L12: "Function call may throw",
      L13: "Throw statement.",
    });
  });

  it('should suppress assignment calls within @it-throws functions', () => {
    const code = stripLineNumbers`
1 | function returnsValue() {
2 |   if (Math.random() > 0.5) throw new Error("random error");
3 |   return "value";
4 | }
5 | 
6 | // @it-throws
7 | function suppressedFunction() {
8 |   const result = returnsValue(); // This call should be suppressed
9 |   return result;
10| }
11| 
12| function normalFunction() {
13|   const result = returnsValue(); // This call should NOT be suppressed
14|   return result;
15| }`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      "L1": "Function returnsValue may throw: {Error}",
      "L2": "Throw statement.",
      "L12": [
        "Function normalFunction may throw: {Error}",
        "Throw statement."
      ],
      "L13": "Function call may throw: {Error}."
    });
  });

  it('should work with the original user scenario', () => {
    const code = stripLineNumbers`
1 | /**
2 |  * @throws {DatabaseError} Database errors
3 |  */
4 | export function saveToDatabase(data: any) {
5 |   if (!data) throw new DatabaseError("No data to save");
6 |   if (Math.random() > 0.8) throw new DatabaseError("Database connection failed");
7 |   return { saved: true, id: "12345" };
8 | }
9 | 
10| // @it-throws
11| export function processUserWithIncompleteCatch(userId: string, token: string) {
12|   try {
13|     const user = fetchUserFromNetwork(userId);
14|     const result = saveToDatabase(user); // This should be suppressed
15|     return result;
16|   } catch (e) {
17|     throw e; // This should be suppressed
18|   }
19| }
20| 
21| export function processUserWithoutSuppression(userId: string, token: string) {
22|   try {
23|     const user = fetchUserFromNetwork(userId);
24|     const result = saveToDatabase(user); // This should NOT be suppressed
25|     return result;
26|   } catch (e) {
27|     throw e; // This should NOT be suppressed
28|   }
29| }
30| 
31| function fetchUserFromNetwork(userId: string) { return {}; }
32| class DatabaseError extends Error {}`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      "L21": "Function processUserWithoutSuppression may throw: {DatabaseError}",
      "L24": "Function call may throw: {DatabaseError}.",
      "L27": "Throw statement."
    });
  });
});