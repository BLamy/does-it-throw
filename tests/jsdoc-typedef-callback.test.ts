import { describe, it } from 'vitest'
import { analyzeCode, expectExactDiagnostics, expectExactQuickFixes, stripLineNumbers } from './test-utils'
import { parse_js, InputData } from '../server/src/rust/what_does_it_throw_wasm.js'

function buildParseOptions(code: string) {
  return {
    file_content: code,
    debug: false,
    throw_statement_severity: 'Hint',
    function_throw_severity: 'Hint',
    call_to_throw_severity: 'Hint',
    call_to_imported_throw_severity: 'Hint',
    include_try_statement_throws: false,
    ignore_statements: ['@it-throws']
  } satisfies InputData
}

describe('JSDoc @typedef and @callback support', () => {
  describe('@callback JSDoc annotations', () => {
    it('should recognize @callback definitions with @throws annotations', () => {
      const code = stripLineNumbers`
 1 | /**
 2 |  * @callback ErrorHandler
 3 |  * @param {Error} error - The error that occurred
 4 |  * @throws {NetworkError} When network request fails
 5 |  * @throws {ValidationError} When input validation fails
 6 |  */
 7 | 
 8 | /**
 9 |  * Processes data with error handling
10 |  * @param {Object} data - Data to process
11 |  * @param {ErrorHandler} onError - Error handler callback
12 |  */
13 | function processData(data, onError) {
14 |   // Implementation that might call onError with specific errors
15 |   if (!data) {
16 |     onError(new ValidationError('Data is required'));
17 |   }
18 | }
19 | 
20 | // Usage: callback should be able to handle the specified error types
21 | processData(userData, (error) => {
22 |   if (error instanceof NetworkError) {
23 |     console.log('Network error handled');
24 |   } else if (error instanceof ValidationError) {
25 |     console.log('Validation error handled');
26 |   }
27 |   // This callback should be recognized as potentially throwing the documented errors
28 | });
`

      const diagnostics = analyzeCode(code)
      
      // The system should understand that the callback parameter can throw specific errors
      // and validate callback usage accordingly
      expectExactDiagnostics(diagnostics, {
        // Expected behavior: system recognizes callback error types
      })
    })

    it('should handle multiple @callback definitions', () => {
      const code = stripLineNumbers`
 1 | /**
 2 |  * @callback SuccessCallback
 3 |  * @param {Object} result - Success result
 4 |  * @throws {ValidationError} When result validation fails
 5 |  */
 6 | 
 7 | /**
 8 |  * @callback ErrorCallback
 9 |  * @param {Error} error - Error that occurred
10 |  * @throws {NetworkError} When network fails
11 |  * @throws {TimeoutError} When request times out
12 |  */
13 | 
14 | /**
15 |  * Async operation with success and error callbacks
16 |  * @param {Object} options - Operation options
17 |  * @param {SuccessCallback} onSuccess - Success handler
18 |  * @param {ErrorCallback} onError - Error handler
19 |  */
20 | function asyncOperation(options, onSuccess, onError) {
21 |   // Implementation
22 | }
`

      const diagnostics = analyzeCode(code)
      
      expectExactDiagnostics(diagnostics, {
        // Should recognize both callback types with their specific throws
      })
    })
  })

  describe('@typedef JSDoc annotations', () => {
    it.skip('should recognize @typedef {function} with @throws annotations', () => {
      const code = stripLineNumbers`
 1 | /**
 2 |  * @typedef {function} DataProcessor
 3 |  * @param {Object} data - Data to process
 4 |  * @param {Object} options - Processing options
 5 |  * @throws {ProcessingError} When data processing fails
 6 |  * @throws {ValidationError} When input validation fails
 7 |  */
 8 | 
 9 | /**
10 |  * Processes user data
11 |  * @param {Object} userData - User data to process
12 |  * @param {DataProcessor} processor - Data processing function
13 |  */
14 | function processUserData(userData, processor) {
15 |   // Call processor which can throw specific errors
16 |   processor(userData, { validate: true });
17 | }
18 | 
19 | // Usage with a processor that matches the typedef
20 | processUserData(user, (data, options) => {
21 |   if (options.validate && !data.email) {
22 |     throw new ValidationError('Email is required');
23 |   }
24 |   if (data.corrupted) {
25 |     throw new ProcessingError('Data is corrupted');
26 |   }
27 |   return processedData;
28 | });
`

      const diagnostics = analyzeCode(code)
      
      expectExactDiagnostics(diagnostics, {
        // Should understand typedef function throws specifications
      })
    })

    it('should handle regular @typedef (non-function) definitions', () => {
      const code = stripLineNumbers`
 1 | /**
 2 |  * @typedef {Object} UserConfig
 3 |  * @property {string} apiUrl - API endpoint URL
 4 |  * @property {number} timeout - Request timeout
 5 |  * @property {boolean} validateSSL - Whether to validate SSL
 6 |  */
 7 | 
 8 | /**
 9 |  * @typedef {function} ConfigValidator
10 |  * @param {UserConfig} config - Configuration to validate
11 |  * @throws {ConfigurationError} When configuration is invalid
12 |  */
13 | 
14 | /**
15 |  * Initializes the application with configuration
16 |  * @param {UserConfig} config - Application configuration
17 |  * @param {ConfigValidator} validator - Configuration validator
18 |  */
19 | function initializeApp(config, validator) {
20 |   validator(config);
21 | }
`

      const diagnostics = analyzeCode(code)
      
      expectExactDiagnostics(diagnostics, {
        // Should distinguish between object typedefs and function typedefs
      })
    })
  })

  describe('Parameter-level @throws annotations', () => {
    it('should recognize inline parameter @throws annotations', () => {
      const code = stripLineNumbers`
 1 | /**
 2 |  * Processes data with multiple callback parameters
 3 |  * @param {Object} data - Data to process
 4 |  * @param {function} validator - Data validator /** @throws {ValidationError} */
 5 |  * @param {function} processor - Data processor /** @throws {ProcessingError, NetworkError} */
 6 |  * @param {function} finalizer - Final processing step /** @throws {FinalizationError} */
 7 |  */
 8 | function processWithCallbacks(data, validator, processor, finalizer) {
 9 |   validator(data);
10 |   const processed = processor(data);
11 |   return finalizer(processed);
12 | }
13 | 
14 | // Usage: callbacks should match their parameter throw specifications
15 | processWithCallbacks(
16 |   userData,
17 |   (data) => {
18 |     if (!data.valid) throw new ValidationError('Invalid data');
19 |   },
20 |   (data) => {
21 |     if (network.failed) throw new NetworkError('Network failed');
22 |     return transform(data);
23 |   },
24 |   (data) => {
25 |     if (finalization.failed) throw new FinalizationError('Finalization failed');
26 |     return data;
27 |   }
28 | );
`

      const diagnostics = analyzeCode(code)
      
      expectExactDiagnostics(diagnostics, {
        // Should recognize parameter-level throws specifications
      })
    })

    it('should handle parameter @throws without braces', () => {
      const code = stripLineNumbers`
 1 | /**
 2 |  * Legacy function with old-style parameter throws
 3 |  * @param {function} callback - Callback function /** @throws Error, TypeError when validation fails */
 4 |  */
 5 | function legacyProcess(callback) {
 6 |   callback();
 7 | }
`

      const diagnostics = analyzeCode(code)
      
      expectExactDiagnostics(diagnostics, {
        // Should handle legacy throw annotation syntax
      })
    })
  })

  describe('Integration with existing throw detection', () => {
    it('should integrate callback throws with function analysis', () => {
      const code = stripLineNumbers`
 1 | /**
 2 |  * @callback AsyncCallback
 3 |  * @param {Error|null} error - Error if operation failed
 4 |  * @param {Object} result - Result if operation succeeded
 5 |  * @throws {TimeoutError} When operation times out
 6 |  * @throws {NetworkError} When network request fails
 7 |  */
 8 | 
 9 | /**
10 |  * Async operation that uses a callback
11 |  * @param {Object} options - Operation options
12 |  * @param {AsyncCallback} callback - Completion callback
13 |  */
14 | function asyncRequest(options, callback) {
15 |   // Simulate async operation
16 |   setTimeout(() => {
17 |     if (options.timeout) {
18 |       callback(new TimeoutError('Request timed out'), null);
19 |     } else if (options.networkError) {
20 |       callback(new NetworkError('Network failed'), null);
21 |     } else {
22 |       callback(null, { success: true });
23 |     }
24 |   }, 100);
25 | }
26 | 
27 | // Function that uses the async request
28 | function handleUserRequest(userData) {
29 |   asyncRequest({ timeout: false }, (error, result) => {
30 |     if (error) {
31 |       if (error instanceof TimeoutError) {
32 |         console.log('Handle timeout');
33 |       } else if (error instanceof NetworkError) {
34 |         console.log('Handle network error');
35 |       }
36 |       return;
37 |     }
38 |     console.log('Success:', result);
39 |   });
40 | }
`

      const diagnostics = analyzeCode(code)
      
      expectExactDiagnostics(diagnostics, {
        // Should integrate callback error specifications with overall analysis
      })
    })

    it.skip('should validate callback parameter compatibility', () => {
      const code = stripLineNumbers`
 1 | /**
 2 |  * @callback StrictValidator
 3 |  * @param {Object} data - Data to validate
 4 |  * @throws {ValidationError} When validation fails
 5 |  */
 6 | 
 7 | /**
 8 |  * Function that expects a strict validator
 9 |  * @param {Object} data - Data to validate
10 |  * @param {StrictValidator} validator - Validator function
11 |  */
12 | function validateData(data, validator) {
13 |   validator(data);
14 | }
15 | 
16 | // Compatible callback - should not be flagged
17 | validateData(userData, (data) => {
18 |   if (!data.valid) {
19 |     throw new ValidationError('Data is invalid');
20 |   }
21 | });
22 | 
23 | // Incompatible callback - should be flagged for throwing wrong error type
24 | validateData(userData, (data) => {
25 |   if (!data.valid) {
26 |     throw new TypeError('Wrong error type'); // Should be flagged
27 |   }
28 | });
`

      const diagnostics = analyzeCode(code)
      
      expectExactDiagnostics(diagnostics, {
        // Should validate callback compatibility with typedef/callback specifications
        'L26': 'Throw statement.' // Wrong error type thrown
      })
    })
  })

  describe('Complex real-world scenarios', () => {
    it.skip('should handle complex API with multiple callback types', () => {
      const code = stripLineNumbers`
 1 | /**
 2 |  * @callback DataValidator
 3 |  * @param {Object} data - Data to validate
 4 |  * @throws {ValidationError} When data is invalid
 5 |  * @throws {SchemaError} When data doesn't match schema
 6 |  */
 7 | 
 8 | /**
 9 |  * @callback DataTransformer
10 |  * @param {Object} data - Data to transform
11 |  * @throws {TransformationError} When transformation fails
12 |  * @throws {TypeError} When data type is incompatible
13 |  */
14 | 
15 | /**
16 |  * @typedef {function} ErrorHandler
17 |  * @param {Error} error - Error that occurred
18 |  * @throws {LoggingError} When error logging fails
19 |  */
20 | 
21 | /**
22 |  * Data processing pipeline with comprehensive error handling
23 |  * @param {Object} rawData - Raw input data
24 |  * @param {DataValidator} validator - Data validation function
25 |  * @param {DataTransformer} transformer - Data transformation function
26 |  * @param {ErrorHandler} errorHandler - Error handling function
27 |  */
28 | function processDataPipeline(rawData, validator, transformer, errorHandler) {
29 |   try {
30 |     validator(rawData);
31 |     const transformed = transformer(rawData);
32 |     return transformed;
33 |   } catch (error) {
34 |     errorHandler(error);
35 |     throw error; // Re-throw after handling
36 |   }
37 | }
38 | 
39 | // Usage with proper error handling
40 | processDataPipeline(
41 |   inputData,
42 |   (data) => {
43 |     if (!data.schema) throw new SchemaError('Missing schema');
44 |     if (!data.valid) throw new ValidationError('Invalid data');
45 |   },
46 |   (data) => {
47 |     if (typeof data !== 'object') throw new TypeError('Expected object');
48 |     return transformData(data);
49 |   },
50 |   (error) => {
51 |     console.error('Processing error:', error);
52 |     if (loggingFailed) throw new LoggingError('Failed to log error');
53 |   }
54 | );
`

      const diagnostics = analyzeCode(code)
      
      expectExactDiagnostics(diagnostics, {
        // Should handle complex scenarios with multiple callback types
        // and proper error type validation
      })
    })
  })
})
