import { describe, it, expect } from "vitest";
import { analyzeCode, stripLineNumbers } from "./test-utils";

describe("Integration: File-level disable in practice", () => {
  it("should not generate warnings for test utility functions", () => {
    // Simulate the kind of utility functions that might exist in test-utils.ts
    const testUtilsCode = stripLineNumbers`
1 | // @it-throws-disable
2 | 
3 | export function analyzeCode(code: string) {
4 |   const result = parse_js({ file_content: code });
5 |   if (!result.diagnostics) {
6 |     throw new Error("Failed to parse code");
7 |   }
8 |   return result.diagnostics;
9 | }
10| 
11| export function expectDiagnostic(diagnostics: any[], line: number, message: string) {
12|   const diagnostic = diagnostics.find(d => d.line === line);
13|   if (!diagnostic) {
14|     throw new Error(\`Expected diagnostic at line \${line}, but found none\`);
15|   }
16|   if (!diagnostic.message.includes(message)) {
17|     throw new Error(\`Expected message to contain "\${message}"\`);
18|   }
19| }
20|
21| export function stripLineNumbers(template: any) {
22|   // Implementation that might throw
23|   if (!template) {
24|     throw new TypeError("Template is required");
25|   }
26|   return template.toString();
27| }`;

    const diagnostics = analyzeCode(testUtilsCode);

    // Should have NO diagnostics because of @it-throws-disable
    expect(diagnostics).toHaveLength(0);
  });

  it("should work with real TypeScript syntax and error handling patterns", () => {
    const realWorldCode = stripLineNumbers`
1 | // @it-throws-disable
2 | 
3 | import { SomeType } from './types';
4 | 
5 | interface ParseOptions {
6 |   debug?: boolean;
7 |   severity?: string;
8 | }
9 | 
10| export class TestHelper {
11|   private options: ParseOptions;
12|   
13|   constructor(options: ParseOptions = {}) {
14|     this.options = options;
15|   }
16|   
17|   validateInput(input: any): string {
18|     if (typeof input !== 'string') {
19|       throw new TypeError('Input must be a string');
20|     }
21|     if (input.length === 0) {
22|       throw new Error('Input cannot be empty');
23|     }
24|     return input;
25|   }
26|   
27|   processWithErrorHandling(code: string): any {
28|     try {
29|       return this.parseCode(code);
30|     } catch (error) {
31|       if (this.options.debug) {
32|         console.error('Parse failed:', error);
33|       }
34|       throw new Error(\`Processing failed: \${error.message}\`);
35|     }
36|   }
37|   
38|   private parseCode(code: string): any {
39|     if (!code.trim()) {
40|       throw new SyntaxError('Code cannot be empty');
41|     }
42|     return { parsed: true };
43|   }
44| }
45|
46| export function createHelper(options?: ParseOptions): TestHelper {
47|   if (options && typeof options !== 'object') {
48|     throw new TypeError('Options must be an object');
49|   }
50|   return new TestHelper(options);
51| }`;

    const diagnostics = analyzeCode(realWorldCode);

    // Should have NO diagnostics because of @it-throws-disable
    expect(diagnostics).toHaveLength(0);
  });

  it("should suppress all types of diagnostics including function calls", () => {
    const codeWithCalls = stripLineNumbers`
1 | // @it-throws-disable
2 | 
3 | function helperThatThrows() {
4 |   throw new Error("helper error");
5 | }
6 | 
7 | function anotherHelper() {
8 |   throw new TypeError("another error");
9 | }
10|
11| export function mainUtility() {
12|   // This would normally generate "function call may throw" warnings
13|   helperThatThrows();
14|   anotherHelper();
15|   
16|   // This would normally generate function-level warnings
17|   throw new RangeError("main utility error");
18| }
19|
20| export function complexUtility() {
21|   try {
22|     mainUtility();
23|   } catch (e) {
24|     // Re-throwing would normally be flagged
25|     throw new Error(\`Complex error: \${e.message}\`);
26|   }
27| }`;

    const diagnostics = analyzeCode(codeWithCalls);

    // Should have NO diagnostics at all - no function warnings, no call warnings, no throw warnings
    expect(diagnostics).toHaveLength(0);
  });
});