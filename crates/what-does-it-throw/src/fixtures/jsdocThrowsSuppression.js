// Test fixtures for enhanced does-it-throw functionality
// These examples demonstrate the new error tracking and throws annotation features

// 1. Basic undocumented functions 
function basicErrorThrow() { // should be flagged
  throw new Error("Something went wrong"); // should be flagged
}

/**
 * 
 */
function basicTypeErrorThrow() { // should be flagged
  throw new TypeError("Expected a string"); // should be flagged
}

function customErrorThrow() { // should be flagged
  throw new ValidationError("Custom validation failed"); // should be flagged
}

// 2. Documented functions - should NOT be flagged for function-level diagnostics
/**
 * @throws {Error}
 */
function documentedErrorThrow() { // should not be flagged
  throw new Error("This error is documented"); // should not be flagged
}

/**
 * @throws {TypeError}
 */
function documentedTypeErrorThrow() { // should not be flagged
  throw new TypeError("This type error is documented"); // should not be flagged
}

/**
 * @throws {ValidationError}
 */
function documentedCustomErrorThrow() { // should not be flagged
  throw new ValidationError("This custom error is documented"); // should not be flagged
}

// 3. Functions with multiple documented error types
/**
 * @throws {TypeError} - Input must be a string
 * @throws {ValidationError} - Input cannot be empty
 */
function multipleDocumentedErrors(input) { // should NOT be flagged
  if (typeof input !== 'string') {
    throw new TypeError("Input must be a string"); // should not be flagged
  }
  
  if (input.length === 0) {
    throw new ValidationError("Input cannot be empty"); // should not be flagged
  }
  
  return input.toUpperCase();
}

// 4. Partially documented functions - should flag undocumented throws
/**
 * @throws {Error}
 */
function partiallyDocumented(x) { // should be flagged
  if (x) {
    throw new Error("This is documented"); // should not be flagged
  }
  throw new TypeError("This is NOT documented"); // Should be flagged
}

/**
 * @throws {TypeError}
 */
function anotherPartiallyDocumented(x) { // should be flagged
  if (x) {
    throw new TypeError("This is documented"); // should not be flagged
  } else if (x === 0) {
    throw new RangeError("This is NOT documented"); // Should be flagged
  } else {
    throw new ValidationError("This is also NOT documented"); // Should be flagged
  }
}

// 5. Different throw patterns
function throwStringLiteral() { // should be flagged
  throw "This is a string error"; // should be flagged
}

function throwVariable() { // should be flagged
  const existingError = new Error("Existing error"); // should not be flagged
  throw existingError; // should be flagged
}

function throwExpression() { // should be flagged
  throw new Error(`Dynamic error: ${Date.now()}`); // should be flagged
}

// 6. Function call chains - testing cascade behavior
function callsUndocumentedFunction() { // should be flagged
  return basicErrorThrow(); // Should be flagged - calls undocumented throwing function
} // BUG: not flagged when it should be

function callsDocumentedFunction() { // should be flagged
  return documentedErrorThrow(); // Should be flagged - calls documented function but this function is not documented
} // BUG: not flagged when it should be

/**
 * @throws {Error}
 */
function properlyDocumentedCallChain() {
  return documentedErrorThrow(); // Should NOT be flagged - both functions properly documented
}

/**
 * @throws {TypeError}
 * @throws {ValidationError}
 */
function callsMultipleErrorFunction() { // should NOT be flagged
  return multipleDocumentedErrors("test"); // Should NOT be flagged - properly documented
}

function callsMultipleErrorFunctionUndocumented() { // BUG: not flagged when it should be
  return multipleDocumentedErrors("test"); // Should be flagged - calls documented function but this is not documented
}

// 7. Nested function calls
/**
 * @throws {ValidationError}
 * @throws {TypeError}
 */
function processInput(data) { // should NOT be flagged
  return multipleDocumentedErrors(data); // should NOT be flagged
}

function handleData(input) { // should be flagged
  return processInput(input); // Should be flagged - calls function that throws ValidationError
}

/**
 * @throws {ValidationError}
 */
function properlyHandleData(input) { // should NOT be flagged
  return processInput(input); // Should NOT be flagged - properly documented
}

// 8. Arrow functions
const arrowBasicThrow = () => { // should be flagged
  throw new Error("Arrow function error"); // should be flagged
};

/**
 * @throws {TypeError}
 */
const documentedArrowThrow = () => { // should not be flagged
  throw new TypeError("Documented arrow function error"); // should not be flagged
};

const arrowCallsDocumented = () => { // Should be flagged
  return documentedArrowThrow(); // Should be flagged
}; // BUG: Flagged when it should not be

/**
 * @throws {TypeError}
 */
const properlyDocumentedArrowCall = () => { // Should NOT be flagged
  return documentedArrowThrow(); // Should NOT be flagged
};

// 9. Class methods
class TestClass {
  undocumentedMethod() { // should be flagged
    throw new Error("Class method error"); // should be flagged
  }

  /**
   * @throws {TypeError}
   */
  documentedMethod() { // should not be flagged
    throw new TypeError("Documented class method error"); // should not be flagged
  }

  callsDocumentedMethod() {// should be flagged
    return this.documentedMethod(); // Should be flagged
  } // BUG: Flagged when it should not be

  /**
   * @throws {TypeError}
   */
  properlyCallsDocumentedMethod() { // should NOT be flagged
    return this.documentedMethod(); // Should NOT be flagged
  }
}

// 10. Real-world validation scenario
class ValidationError extends Error {
  constructor(message) {
    super(message);
    this.name = 'ValidationError';
  }
}

class AuthenticationError extends Error {
  constructor(message) {
    super(message);
    this.name = 'AuthenticationError';
  }
}

/**
 * @throws {TypeError} - Input must be an object
 * @throws {ValidationError} - Username must be at least 3 characters
 * @throws {ValidationError} - Valid email is required
 */
function validateUserInput(input) {
  if (typeof input !== 'object' || input === null) {
    throw new TypeError('Input must be an object');
  }
  
  if (!input.username || input.username.length < 3) {
    throw new ValidationError('Username must be at least 3 characters');
  }
  
  if (!input.email || !input.email.includes('@')) {
    throw new ValidationError('Valid email is required');
  }
  
  return input;
}

/**
 * @throws {AuthenticationError} - Authentication token is required
 */
function authenticateUser(credentials) {
  if (!credentials.token) {
    throw new AuthenticationError('Authentication token is required');
  }
  
  // Simulate authentication logic
  if (credentials.token !== 'valid-token') {
    throw new AuthenticationError('Invalid authentication token');
  }
  
  return { authenticated: true };
}


/**
 * @throws {TypeError} - Input must be an object
 * @throws {ValidationError} - Username must be at least 3 characters
 * @throws {ValidationError} - Valid email is required
 * @throws {AuthenticationError} - Authentication token is required
 */
function processUserRegistration(userData) { // should NOT be flagged
  const validated = validateUserInput(userData); // should NOT be flagged
  const authenticated = authenticateUser(userData); // should NOT be flagged
  return { user: validated, auth: authenticated };
}

function handleUserRegistration(req) { // Should be flagged - not documented
  return processUserRegistration(req.body); 
}

/**
 * @throws {TypeError} - Input must be an object
 * @throws {ValidationError} - Username must be at least 3 characters
 * @throws {AuthenticationError} - Authentication token is required
 */
function properlyHandleUserRegistration(req) {
  return processUserRegistration(req.body); // Should NOT be flagged - properly documented
} 