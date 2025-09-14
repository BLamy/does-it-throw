import { describe, it, expect } from "vitest";
import {
  analyzeCode,
  loadFixture,
  expectExactDiagnostics,
  stripLineNumbers,
} from "./test-utils";

describe("Real-world fixture tests for comprehensive functionality", () => {
  describe("TypeScript (.ts) fixtures", () => {
    it.skip("should properly handle callExpr.ts fixture - function call detection", () => {
      const code = loadFixture("callExpr.ts");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L25: [
          "Function SomeRandomCall2 may throw: {Error}",
          "Throw statement.",
        ],
        L29: "Function SomeThrow may throw: {Error}",
        L30: "Throw statement.",
        L33: "Function SomeThrow2 may throw: {Error}",
        L34: "Throw statement.",
        L47: [
          "Function call may throw: {Error}.",
          "Function onInitialized may throw",
        ],
        L48: "Function call may throw: {Error}.",
        L51: "Anonymous function may throw: {Error}",
        L52: "Throw statement.",
        L55: "Anonymous function may throw: {Error}",
        L56: "Throw statement.",
        L60: "Function call may throw: {Error}.",
        L61: "Function call may throw: {Error}.",
        L64: "Anonymous function may throw: {Error}",
        L65: "Throw statement.",
        L70: [
          "Function call may throw: {Error}.",
          "Function getter test may throw",
        ],
        L76: "Function call may throw: {Error}.",
        L77: "Function call may throw: {Error}.",
      });
    });
    it("should properly handle class.ts fixture - class methods and constructors", () => {
      const code = loadFixture("class.ts");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L4: "Function <constructor> may throw: {Error}",
        L5: "Throw statement.",
        L8: "Function someMethodThatThrows may throw: {Error}",
        L9: "Throw statement.",
        L16: "Function someMethodThatThrows2 may throw: {Error}",
        L18: "Throw statement.",
        L22: "Function nestedThrow may throw: {Error}",
        L26: "Throw statement.",
        L36: ["Function call may throw", "Function callNestedThrow may throw"],
        L42: ["Function call may throw", "Function _somethingCall may throw"],
        L47: ["Function call may throw", "Function somethingCall may throw"],
        L52: ["Function call may throw", "Function _somethingCall2 may throw"],
        L57: ["Function call may throw", "Function somethingCall2 may throw"],
      });
    });
    it("should properly handle comprehensiveErrorFlow.ts fixture - complex error patterns", () => {
      const code = loadFixture("comprehensiveErrorFlow.ts");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L168: "Function processUserWithIncompleteCatch may throw: {DatabaseError}",
        L190: "Throw statement.",
      });
    });
    it("should properly handle exports.ts fixture - export patterns", () => {
      const code = loadFixture("exports.ts");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L7: "Function hiKhue may throw: {Error}",
        L8: "Throw statement.",
        L11: "Function someConstThatThrows may throw: {Error}",
        L12: "Throw statement.",
        L26: "Function _ConstThatThrows may throw: {Error}",
        L27: "Throw statement.",
        L30: [
          "Function callToConstThatThrows may throw: {Error}",
          "Throw statement.",
        ],
        L31: "Function call may throw: {Error}.",
        L35: "Function someConstThatThrows2 may throw: {Error}",
        L37: "Throw statement.",
        L41: [
          "Function callToConstThatThrows2 may throw: {Error}",
          "Throw statement.",
        ],
        L42: "Function call may throw: {Error}.",
        L46: [
          "Function callToConstThatThrows3 may throw: {Error}",
          "Throw statement.",
        ],
        L51: "Function call may throw: {Error}.",
        L55: [
          "Function callToConstThatThrows4 may throw: {Error}",
          "Throw statement.",
        ],
        L56: "Function call may throw: {Error}.",
      });
    });
    it.skip("should properly handle ignoreStatements.ts fixture - proximity-based suppression", () => {
      const code = loadFixture("ignoreStatements.ts");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L21: "Function someMethodThatThrows2 may throw: {Error}",
        L24: "Throw statement.",
        L90: "Function call may throw: {Error}.",
      });
    });
    it("should properly handle importIdentifiers.ts fixture - import analysis", () => {
      const code = loadFixture("importIdentifiers.ts");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {});
    });
    it("should properly handle jsdocThrowsSupression.ts fixture - JSDoc suppression", () => {
      const code = loadFixture("jsdocThrowsSupression.ts");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L7: "Function basicErrorThrow may throw: {Error}",
        L8: "Throw statement.",
        L14: "Function basicTypeErrorThrow may throw: {TypeError}",
        L15: "Throw statement.",
        L18: "Function customErrorThrow may throw: {ValidationError}",
        L19: "Throw statement.",
        L65: "Function partiallyDocumented may throw: {TypeError}",
        L69: "Throw statement.",
        L75: "Function anotherPartiallyDocumented may throw: {RangeError, ValidationError}",
        L79: "Throw statement.",
        L81: "Throw statement.",
        L86: "Function throwStringLiteral may throw",
        L87: "Throw statement.",
        L90: "Function throwVariable may throw: {variable: existingError}",
        L92: "Throw statement.",
        L95: "Function throwExpression may throw: {Error}",
        L96: "Throw statement.",
        L100: [
          "Function callsUndocumentedFunction may throw: {Error}",
          "Throw statement.",
        ],
        L101: "Function call may throw: {Error}.",
        L104: [
          "Function callsDocumentedFunction may throw: {Error}",
          "Throw statement.",
        ],
        L105: "Function call may throw: {Error}.",
        L124: "Function call may throw: {TypeError, ValidationError}.",
        L148: "Function arrowBasicThrow may throw: {Error}",
        L149: "Throw statement.",
        L159: [
          "Function arrowCallsDocumented may throw: {TypeError}",
          "Throw statement.",
        ],
        L160: "Function call may throw: {TypeError}.",
        L172: "Function undocumentedMethod may throw: {Error}",
        L173: "Throw statement.",
        L183: [
          "Function callsDocumentedMethod may throw: {TypeError}",
          "Throw statement.",
        ],
        L184: "Function call may throw: {TypeError}.",
      });
    });
    it("should properly handle objectLiteral.ts fixture - object literal patterns", () => {
      const code = loadFixture("objectLiteral.ts");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L2: "Function objectLiteralThrow may throw: {Error}",
        L3: "Throw statement.",
        L6: "Function nestedObjectLiteralThrow may throw: {Error}",
        L7: "Throw statement.",
        L13: "Function someExampleThrow may throw: {Error}",
        L14: "Throw statement.",
        L19: [
          "Function call may throw: {Error}.",
          "Function callToLiteral may throw: {Error}",
        ],
        L23: [
          "Function call may throw: {Error}.",
          "Function callToLiteral2 may throw: {Error}",
        ],
        L26: ["Function callToLiteral3 may throw: {Error}", "Throw statement."],
        L27: "Function call may throw: {Error}.",
        L28: "Function call may throw: {Error}.",
      });
    });
    it.skip("should properly handle returnStatement.ts fixture - return statement patterns", () => {
      const code = loadFixture("returnStatement.ts");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L1: "Function someThrow may throw: {Error}",
        L4: "Throw statement.",
        L8: "Throw statement.",
        L13: "Function badMethod may throw: {Error}",
        L14: "Throw statement.",
        L21: "Function call may throw: {Error}.",
        L22: "Function call may throw: {Error}.",
        L23: "Function call may throw: {Error}.",
        L24: "Function call may throw: {Error}.",
        L25: "Function call may throw: {Error}.",
        L30: "Function call may throw: {Error}.",
      });
    });
    it("should properly handle sample.ts fixture - basic patterns", () => {
      const code = loadFixture("sample.ts");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L3: "Function someConstThatThrows may throw: {Error}",
        L4: "Throw statement.",
        L7: [
          "Function callToConstThatThrows4 may throw: {Error}",
          "Throw statement.",
        ],
        L8: "Function call may throw",
      });
    });
    it("should properly handle something.ts fixture - miscellaneous patterns", () => {
      const code = loadFixture("something.ts");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L1: "Function SomeThrow may throw: {Error}",
        L2: "Throw statement.",
        L5: "Function Something may throw: {Error}",
        L6: "Throw statement.",
        L15: "Function objectLiteralThrow may throw: {Error}",
        L16: "Throw statement.",
      });
    });
    it.skip("should properly handle spreadExpr.ts fixture - spread expressions", () => {
      const code = loadFixture("spreadExpr.ts");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L6: "Function _contextFromWorkflow may throw: {Error}",
        L7: "Throw statement.",
        L13: "Function call may throw: {Error}.",
        L22: "Function _contextFromWorkflow may throw: {Error}",
        L23: "Throw statement.",
        L26: [
          "Function someCallToThrow may throw: {Error}",
          "Throw statement.",
        ],
        L27: "Function call may throw: {Error}.",
      });
    });
    it("should properly handle switchStatement.ts fixture - switch patterns", () => {
      const code = loadFixture("switchStatement.ts");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L3: "Function someRandomThrow may throw: {Error}",
        L4: "Throw statement.",
        L7: "Anonymous function may throw: {Error}",
        L11: "Throw statement.",
        L24: ["Function call may throw", "Function createServer may throw"],
      });
    });
    it("should properly handle test_throw_e.ts fixture - specific throw patterns", () => {
      const code = loadFixture("test_throw_e.ts");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {});
    });
    it("should properly handle tryStatement.ts fixture - try-catch patterns", () => {
      const code = loadFixture("tryStatement.ts");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L36: "Function someMethodThatThrows2 may throw: {Error}",
        L38: "Throw statement.",
      });
    });
    it("should properly handle tryStatementNested.ts fixture - nested try-catch patterns", () => {
      const code = loadFixture("tryStatementNested.ts");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L2: "Function throwInsideCatch may throw: {Error}",
        L6: "Throw statement.",
        L10: "Function parentCatchThatisNotCaught may throw: {Error}",
        L19: "Throw statement.",
      });
    });
  });

  describe("JavaScript (.js) fixtures", () => {
    it("should properly handle class.js fixture - JavaScript class patterns", () => {
      const code = loadFixture("class.js");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L4: "Function <constructor> may throw: {Error}",
        L5: "Throw statement.",
        L8: "Function someMethodThatThrows may throw: {Error}",
        L9: "Throw statement.",
        L16: "Function someMethodThatThrows2 may throw: {Error}",
        L18: "Throw statement.",
        L22: "Function nestedThrow may throw: {Error}",
        L26: "Throw statement.",
        L36: ["Function call may throw", "Function callNestedThrow may throw"],
        L42: ["Function call may throw", "Function _somethingCall may throw"],
        L47: ["Function call may throw", "Function somethingCall may throw"],
        L52: ["Function call may throw", "Function _somethingCall2 may throw"],
        L57: ["Function call may throw", "Function somethingCall2 may throw"],
      });
    });
    it("should properly handle exports.js fixture - JavaScript export patterns", () => {
      const code = loadFixture("exports.js");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L3: "Function hiKhue may throw: {Error}",
        L4: "Throw statement.",
        L7: "Function someConstThatThrows may throw: {Error}",
        L8: "Throw statement.",
        L16: "Function _ConstThatThrows may throw: {Error}",
        L17: "Throw statement.",
        L20: [
          "Function callToConstThatThrows may throw: {Error}",
          "Throw statement.",
        ],
        L21: "Function call may throw",
        L24: "Function someConstThatThrows2 may throw: {Error}",
        L26: "Throw statement.",
        L30: [
          "Function callToConstThatThrows2 may throw: {Error}",
          "Throw statement.",
        ],
        L31: "Function call may throw",
        L34: [
          "Function callToConstThatThrows3 may throw: {Error}",
          "Throw statement.",
        ],
        L35: "Function call may throw",
        L38: [
          "Function callToConstThatThrows4 may throw: {Error}",
          "Throw statement.",
        ],
        L39: "Function call may throw",
      });
    });
    it.skip("should properly handle ignoreStatements.js fixture - JavaScript suppression patterns", () => {
      const code = loadFixture("ignoreStatements.js");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {});
    });
    it("should properly handle jsdocThrowsSuppression.js fixture - JSDoc suppression in JavaScript", () => {
      const code = loadFixture("jsdocThrowsSuppression.js");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L5: "Function basicErrorThrow may throw: {Error}",
        L6: "Throw statement.",
        L12: "Function basicTypeErrorThrow may throw: {TypeError}",
        L13: "Throw statement.",
        L16: "Function customErrorThrow may throw: {ValidationError}",
        L17: "Throw statement.",
        L63: "Function partiallyDocumented may throw: {TypeError}",
        L67: "Throw statement.",
        L73: "Function anotherPartiallyDocumented may throw: {RangeError, ValidationError}",
        L77: "Throw statement.",
        L79: "Throw statement.",
        L84: "Function throwStringLiteral may throw",
        L85: "Throw statement.",
        L88: "Function throwVariable may throw: {variable: existingError}",
        L90: "Throw statement.",
        L93: "Function throwExpression may throw: {Error}",
        L94: "Throw statement.",
        L98: [
          "Function callsUndocumentedFunction may throw: {Error}",
          "Throw statement.",
        ],
        L99: "Function call may throw: {Error}.",
        L102: [
          "Function callsDocumentedFunction may throw: {Error}",
          "Throw statement.",
        ],
        L103: "Function call may throw: {Error}.",
        L122: "Function call may throw: {TypeError, ValidationError}.",
        L146: "Function arrowBasicThrow may throw: {Error}",
        L147: "Throw statement.",
        L157: [
          "Function arrowCallsDocumented may throw: {TypeError}",
          "Throw statement.",
        ],
        L158: "Function call may throw: {TypeError}.",
        L170: "Function undocumentedMethod may throw: {Error}",
        L171: "Throw statement.",
        L181: [
          "Function callsDocumentedMethod may throw: {TypeError}",
          "Throw statement.",
        ],
        L182: "Function call may throw: {TypeError}.",
      });
    });
    it("should properly handle objectLiteral.js fixture - JavaScript object literal patterns", () => {
      const code = loadFixture("objectLiteral.js");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L2: "Function objectLiteralThrow may throw: {Error}",
        L3: "Throw statement.",
        L6: "Function nestedObjectLiteralThrow may throw: {Error}",
        L7: "Throw statement.",
        L13: "Function someExampleThrow may throw: {Error}",
        L14: "Throw statement.",
        L19: ["Function call may throw", "Function callToLiteral may throw"],
        L23: ["Function call may throw", "Function callToLiteral2 may throw"],
        L26: ["Function callToLiteral3 may throw: {Error}", "Throw statement."],
        L27: "Function call may throw",
        L28: "Function call may throw",
      });
    });
  });

  describe("JSX/TSX fixtures", () => {
    it("should properly handle jsx.jsx fixture - JSX patterns", () => {
      const diagnostics = analyzeCode(stripLineNumbers`
1 | export const someThrow = () => {
2 |   throw new Error('some error')
3 | }
4 | export async function callToThrow() {
5 |   someThrow()
6 |   return <div>some tsx</div>
7 | }
`);

      expectExactDiagnostics(diagnostics, {
        L1: "Function someThrow may throw: {Error}",
        L2: "Throw statement.",
        L4: ["Function callToThrow may throw: {Error}", "Throw statement."],
        L5: "Function call may throw: {Error}.",
      });
    });

    it("should properly handle jsx.jsx fixture - JSX patterns", () => {
      const code = loadFixture("jsx.jsx");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L1: "Function someThrow may throw: {Error}",
        L2: "Throw statement.",
        L4: "Function someThrow2 may throw: {Error}",
        L5: "Throw statement.",
        L8: "Function someTsx may throw: {Error}",
        L10: "Throw statement.",
        L15: "Function someAsyncTsx may throw: {Error}",
        L17: "Throw statement.",
        L22: ["Function callToThrow may throw: {Error}", "Throw statement."],
        L23: "Function call may throw",
        L24: "Function call may throw",
        L28: ["Function someTsxWithJsx may throw: {Error}", "Throw statement."],
        L29: "Function call may throw",
        L30: "Function call may throw",
      });
    });
    it("should properly handle tsx.tsx fixture - TSX patterns", () => {
      const code = loadFixture("tsx.tsx");
      const diagnostics = analyzeCode(code);

      expectExactDiagnostics(diagnostics, {
        L2: "Function someThrow may throw: {Error}",
        L3: "Throw statement.",
        L5: "Function someThrow2 may throw: {Error}",
        L6: "Throw statement.",
        L9: "Function someTsx may throw: {Error}",
        L11: "Throw statement.",
        L16: "Function someAsyncTsx may throw: {Error}",
        L18: "Throw statement.",
        L23: ["Function callToThrow may throw: {Error}", "Throw statement."],
        L24: "Function call may throw",
        L25: "Function call may throw",
        L29: ["Function someTsxWithJsx may throw: {Error}", "Throw statement."],
        L30: "Function call may throw",
        L31: "Function call may throw",
      });
    });
  });
});
