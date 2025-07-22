// Test fixtures for enhanced does-it-throw functionality
// These examples demonstrate the new error tracking and throws annotation features

// 1. Basic undocumented functions - should be flagged
function basicErrorThrow() {
  throw new Error("Something went wrong");
}

/**
 * 
 */
function basicTypeErrorThrow() {
  throw new TypeError("Expected a string");
}

function customErrorThrow() {
  throw new ValidationError("Custom validation failed");
}

// 2. Documented functions - should NOT be flagged for function-level diagnostics
/**
 * @throws {Error}
 */
function documentedErrorThrow() {
  throw new Error("This error is documented");
}

/**
 * @throws {TypeError}
 */
function documentedTypeErrorThrow() {
  throw new TypeError("This type error is documented");
}

/**
 * @throws {ValidationError}
 */
function documentedCustomErrorThrow() {
  throw new ValidationError("This custom error is documented");
}

// 3. Functions with multiple documented error types
/**
 * @throws {TypeError} - Input must be a string
 * @throws {ValidationError} - Input cannot be empty
 */
function multipleDocumentedErrors(input) {
  if (typeof input !== 'string') {
    throw new TypeError("Input must be a string");
  }
  
  if (input.length === 0) {
    throw new ValidationError("Input cannot be empty");
  }
  
  return input.toUpperCase();
}

// 4. Partially documented functions - should flag undocumented throws
/**
 * @throws {Error}
 */
function partiallyDocumented() {
  throw new Error("This is documented");
  throw new TypeError("This is NOT documented"); // Should be flagged
}

/**
 * @throws {TypeError}
 */
function anotherPartiallyDocumented() {
  throw new TypeError("This is documented");
  throw new RangeError("This is NOT documented"); // Should be flagged
  throw new ValidationError("This is also NOT documented"); // Should be flagged
}

// 5. Different throw patterns
function throwStringLiteral() {
  throw "This is a string error";
}

function throwVariable() {
  const existingError = new Error("Existing error");
  throw existingError;
}

function throwExpression() {
  throw new Error(`Dynamic error: ${Date.now()}`);
}

// 6. Function call chains - testing cascade behavior
function callsUndocumentedFunction() {
  return basicErrorThrow(); // Should be flagged - calls undocumented throwing function
}

function callsDocumentedFunction() {
  return documentedErrorThrow(); // Should be flagged - calls documented function but this function is not documented
}

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
function callsMultipleErrorFunction() {
  return multipleDocumentedErrors("test"); // Should NOT be flagged - properly documented
}

function callsMultipleErrorFunctionUndocumented() {
  return multipleDocumentedErrors("test"); // Should be flagged - calls documented function but this is not documented
}

// 7. Nested function calls
/**
 * @throws {ValidationError}
 */
function processInput(data) {
  return multipleDocumentedErrors(data); // OK - both throw ValidationError
}

function handleData(input) {
  return processInput(input); // Should be flagged - calls function that throws ValidationError
}

/**
 * @throws {ValidationError}
 */
function properlyHandleData(input) {
  return processInput(input); // Should NOT be flagged - properly documented
}

// 8. Arrow functions
const arrowBasicThrow = () => {
  throw new Error("Arrow function error");
};

/**
 * @throws {TypeError}
 */
const documentedArrowThrow = () => {
  throw new TypeError("Documented arrow function error");
};

const arrowCallsDocumented = () => {
  return documentedArrowThrow(); // Should be flagged
};

/**
 * @throws {TypeError}
 */
const properlyDocumentedArrowCall = () => {
  return documentedArrowThrow(); // Should NOT be flagged
};

// 9. Class methods
class TestClass {
  undocumentedMethod() {
    throw new Error("Class method error");
  }

  /**
   * @throws {TypeError}
   */
  documentedMethod() {
    throw new TypeError("Documented class method error");
  }

  callsDocumentedMethod() {
    return this.documentedMethod(); // Should be flagged
  }

  /**
   * @throws {TypeError}
   */
  properlyCallsDocumentedMethod() {
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
function processUserRegistration(userData) {
  const validated = validateUserInput(userData);
  const authenticated = authenticateUser(userData);
  return { user: validated, auth: authenticated };
}

function handleUserRegistration(req) {
  return processUserRegistration(req.body); // Should be flagged - not documented
}

/**
 * @throws {TypeError} - Input must be an object
 * @throws {ValidationError} - Username must be at least 3 characters
 * @throws {AuthenticationError} - Authentication token is required
 */
function properlyHandleUserRegistration(req) {
  return processUserRegistration(req.body); // Should NOT be flagged - properly documented
} 