import { describe, it, expect } from 'vitest';
import { analyzeCode, expectExactDiagnostics, stripLineNumbers } from './test-utils';

describe('Unused @it-throws comment detection', () => {
  it('should NOT flag @it-throws comments that are actively suppressing diagnostics', () => {
    const code = stripLineNumbers`
1 | // @it-throws
2 | function testFunction() {
3 |   throw new Error('test');
4 | }`;

    const diagnostics = analyzeCode(code);

    // Should have no diagnostics - both function diagnostic and throw statement should be suppressed
    expectExactDiagnostics(diagnostics, {});
    
    // Specifically check that there are no "Unused @it-throws comment" diagnostics
    expect(diagnostics.every(d => !d.message.includes('Unused @it-throws comment'))).toBe(true);
  });

  it.skip('comprehensive suppression should work for all @it-throws scenarios', () => {
    const code = stripLineNumbers`
1 | // @it-throws
2 | function throwsFunction() {
3 |   throw new Error('this throws');
4 | }
5 | 
6 | // @it-throws
7 | function safeFunction() {
8 |   console.log('this is safe');
9 | }`;

    const diagnostics = analyzeCode(code);

    // With comprehensive suppression, @it-throws comments should not be flagged as unused
    // The key test: no function diagnostics or throw statements for functions with @it-throws
    expectExactDiagnostics(diagnostics, {
      'L6': 'Unused @it-throws comment'
    });
  });

  it('should handle arrow functions with comprehensive suppression', () => {
    const code = stripLineNumbers`
1 | // @it-throws
2 | const arrowThrows = () => {
3 |   throw new Error('arrow function throws');
4 | };
5 | 
6 | // @it-throws
7 | const arrowNoThrows = () => {
8 |   return 'safe';
9 | };`;

    const diagnostics = analyzeCode(code);

    // Key test: @it-throws should suppress diagnostics for throwing arrow functions
    expectExactDiagnostics(diagnostics, {
      'L6': 'Unused @it-throws comment',
    });
  });

  it('should validate comprehensive suppression works end-to-end', () => {
    const code = stripLineNumbers`
1 | function helper() {
2 |   throw new Error('helper throws');
3 | }
4 | 
5 | // @it-throws
6 | function withSuppression() {
7 |   helper(); // This call should be suppressed
8 |   throw new Error('also suppressed');
9 | }`;

    const diagnostics = analyzeCode(code);

    // Helper function should have diagnostics (no @it-throws comment)
    expectExactDiagnostics(diagnostics, {
      'L1': 'helper may throw',
      'L2': 'Throw statement.',
    });
  });

  it('should work with JSDoc and @it-throws combinations', () => {
    const code = stripLineNumbers`
1 | /**
2 |  * @throws {Error} This function throws
3 |  */
4 | function documentedThrows() {
5 |   throw new Error('documented');
6 | }
7 | 
8 | // @it-throws
9 | function commentSuppressed() {
10|   throw new Error('comment suppressed');
11| }`;

    const diagnostics = analyzeCode(code);
    
    expectExactDiagnostics(diagnostics, { });
  });

  it('should handle proximity-based suppression for inline comments near throw statements', () => {
    const code = stripLineNumbers`
1 | function testFunction() {
2 |   if (condition) {
3 |     // @it-throws  
4 |     throw new Error('inline error')
5 |   }
6 | }
7 |
8 | function anotherFunction() {
9 |   console.log('start');
10|   // @it-throws
11|   throw new Error('another error'); 
12| }`;

    const diagnostics = analyzeCode(code);
    
    // Both @it-throws comments should be considered "used" due to proximity to throw statements
    const unusedComments = diagnostics.filter(d => d.message.includes('Unused @it-throws comment'));
    
    // The proximity detection should prevent these comments from being flagged as unused
    // since they are within 3 lines of throw statements
    expect(unusedComments.length).toBe(0);
  });

  it.skip('should detect truly unused @it-throws comments that are far from throw statements', () => {
    const code = stripLineNumbers`
1 | // @it-throws
2 | function safeFunction() {
3 |   console.log('no throws here');
4 | }
5 | 
6 | // @it-throws  
7 | const anotherSafe = () => {
8 |   return 'safe';
9 | }
10|
11| // This comment is far from any throws
12| function distantFunction() {
13|   // @it-throws
14|   console.log('safe operation');
15|   console.log('more safe operations');
16|   console.log('still safe');
17|   console.log('even more safe');
18|   console.log('way down here');
19| }`;

    const diagnostics = analyzeCode(code);

    expectExactDiagnostics(diagnostics, {
      'L1': 'Unused @it-throws comment',
      'L6': 'Unused @it-throws comment',
      'L13': 'Unused @it-throws comment',
    });
  });
});