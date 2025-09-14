// Comprehensive Error Flow Test Fixture
// Tests exhaustive catches (ALL catches are exhaustive by default), error propagation, JSDoc suppression, and re-throw detection

// Custom error classes
export class ValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ValidationError';
  }
}

export class NetworkError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'NetworkError';
  }
}

export class AuthenticationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'AuthenticationError';
  }
}

export class DatabaseError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'DatabaseError';
  }
}

// === BASIC THROWING FUNCTIONS ===
/**
 * @throws {ValidationError} Input validation errors
 */
export function validateUserInput(data: any) {
  if (!data) throw new ValidationError("Data is required");
  if (!data.email) throw new ValidationError("Email is required");
  if (data.email.length < 3) throw new ValidationError("Email too short");
}

/**
 * @throws {NetworkError} Network errors
 */
export function fetchUserFromNetwork(userId: string) {
  if (!userId) throw new NetworkError("User ID is required");
  if (Math.random() > 0.7) throw new NetworkError("Connection timeout");
  return { id: userId, email: "user@example.com" };
}

/**
 * @throws {AuthenticationError} Authentication errors
 */
export function authenticateUser(token: string) {
  if (!token) throw new AuthenticationError("Token is required");
  if (token === "invalid") throw new AuthenticationError("Invalid token");
  return { valid: true, user: "authenticated" };
}

/**
 * @throws {DatabaseError} Database errors
 */
export function saveToDatabase(data: any) {
  if (!data) throw new DatabaseError("No data to save");
  if (Math.random() > 0.8) throw new DatabaseError("Database connection failed");
  return { saved: true, id: "12345" };
}

// === EXHAUSTIVE CATCH EXAMPLES (All catches are exhaustive by default) ===
/**
 * @throws {ValidationError} Input validation errors
 * @throws {NetworkError} Network errors
 * @throws {AuthenticationError} Authentication errors
 */
export function partiallyDocumentedProcessor(userId: string, token: string) {
  validateUserInput({ email: userId });
  fetchUserFromNetwork(userId); // Also throws NetworkError but not documented!
  authenticateUser(token); // Also throws AuthenticationError but not documented!
  return "processed";
}



export const arrowWithErrorHandling = (userId: string) => {
  try {
    return fetchUserFromNetwork(userId);
  } catch (e) {
    if (e instanceof NetworkError) {
      return { error: "network-handled" }; // Effectively caught
    }
  }
};

/**
 * @throws {NetworkError} When network operations fail
 */
export const documentedArrowWithRethrow = (userId: string) => {
  try {
    const user = fetchUserFromNetwork(userId);
    validateUserInput(user);
    return user;
  } catch (e) {
    if (e instanceof ValidationError) {
      console.log("Validation handled");
      return { error: "validation" }; // Caught
    }
    throw e; // NetworkError re-thrown, properly documented
  }
};


/**
 * @throws {NetworkError} Network errors
 * @throws {AuthenticationError} Authentication errors
 * @throws {DatabaseError} Database errors
 */
export function complexErrorHandler(data: any) {
  try {
    validateUserInput(data);
    const user = fetchUserFromNetwork(data.id);
    authenticateUser(data.token);
    saveToDatabase(user);
    return "all-good";
  } catch (e) {
    if (e instanceof ValidationError) {
      console.log("Validation error handled locally");
      return "validation-fixed"; // Effectively caught
    } else if (e instanceof NetworkError) {
      console.log("Network error, rethrowing");
      throw new NetworkError("Enhanced: " + e.message); // Re-thrown
    } else if (e instanceof AuthenticationError) {
      console.log("Auth error, rethrowing");
      throw e; // Re-thrown as-is
    }
    throw e; // DatabaseError falls through and is re-thrown (implicit)
  }
}


// GOOD: Complete catch - handles all possible errors
export function processUserWithCompleteCatch(userId: string, token: string) {
  try {
    const user = fetchUserFromNetwork(userId);
    validateUserInput(user);
    authenticateUser(token);
    const result = saveToDatabase(user);
    return result;
  } catch (e) {
    if (e instanceof ValidationError) {
      console.log("Validation failed:", e.message);
      return { error: "validation", message: e.message };
    } else if (e instanceof NetworkError) {
      console.log("Network failed:", e.message);
      return { error: "network", message: e.message };
    } else if (e instanceof AuthenticationError) {
      console.log("Auth failed:", e.message);
      return { error: "auth", message: e.message };
    } else if (e instanceof DatabaseError) {
      console.log("Database failed:", e.message);
      return { error: "database", message: e.message };
    }
    // All error types are handled - this is complete!
  }
}

// BAD: Incomplete catch - missing DatabaseError handler (will be flagged).
export function processUserWithIncompleteCatch(userId: string, token: string) {
  try {
    const user = fetchUserFromNetwork(userId);
    validateUserInput(user);

    authenticateUser(token);
    // @it-throws
    const result = saveToDatabase(user);
    return result;
  } catch (e) {
    // Escape hatch for unhandled errors
    if (e instanceof ValidationError) {
      console.log("Validation failed:", e.message);
      return { error: "validation", message: e.message };
    } else if (e instanceof NetworkError) {
      console.log("Network failed:", e.message);
      return { error: "network", message: e.message };
    } else if (e instanceof AuthenticationError) {
      console.log("Auth failed:", e.message);
      return { error: "auth", message: e.message };
    }

    throw e; // rethrowing should be flagged if not documented

    // Missing DatabaseError handler - this will be flagged as an error!
    // Solution: Add DatabaseError handler OR use 'throw e' as escape hatch
  }
}

// GOOD: Escape hatch - use `throw e` to explicitly allow unhandled errors to propagate
/**
 * @throws {AuthenticationError} - 
 * @throws {DatabaseError}
 */
export function processUserWithEscapeHatch(userId: string, token: string) {
  try {
    const user = fetchUserFromNetwork(userId);
    validateUserInput(user);
    authenticateUser(token);
    const result = saveToDatabase(user);
    return result;
  } catch (e) {
    if (e instanceof ValidationError) {
      console.log("Validation failed:", e.message);
      return { error: "validation", message: e.message };
    } else if (e instanceof NetworkError) {
      console.log("Network failed:", e.message);
      return { error: "network", message: e.message };
    }
    // Explicit escape hatch - unhandled errors (AuthenticationError, DatabaseError) will propagate
    // This requires the calling function to have @throws documentation
    throw e;
  }
}

// === EFFECTIVELY CAUGHT ERRORS (Should NOT require JSDoc) ===

// GOOD: Effectively catches ValidationError and NetworkError
export function processUserSafely(userId: string) {
  try {
    const user = fetchUserFromNetwork(userId);
    validateUserInput(user);
    return { success: true, user };
  } catch (e) {
    if (e instanceof ValidationError) {
      console.log("Validation handled");
      return { success: false, reason: "validation" }; // Handled, not re-thrown
    } else if (e instanceof NetworkError) {
      console.log("Network handled");
      return { success: false, reason: "network" }; // Handled, not re-thrown
    }
    // Other errors (if any) will propagate
  }
}

// GOOD: Effectively catches all errors from its calls
export function fullyProtectedProcessor(userId: string) {
  try {
    validateUserInput({ email: userId });
    fetchUserFromNetwork(userId);
    return "success";
  } catch (e) {
    if (e instanceof ValidationError) {
      return "validation-handled";
    } else if (e instanceof NetworkError) {
      return "network-handled";
    }
    // All possible errors are handled
  }
}

// === RE-THROWING FUNCTIONS (SHOULD require JSDoc) ===

// GOOD: Re-throws errors - should require JSDoc @throws
/**
 * @throws {NetworkError}
 * @throws {AuthenticationError}
 * @throws {ValidationError}
 */
export function processUserUnsafely(userId: string, token: string) {
  try {
    const user = fetchUserFromNetwork(userId);
    validateUserInput(user);
    authenticateUser(token);
    return { success: true, user };
  } catch (e) {
    if (e instanceof ValidationError) {
      console.log("Validation failed");
      throw new ValidationError("Re-wrapped validation error"); // Re-thrown as new error
    } else if (e instanceof NetworkError) {
      console.log("Network failed");
      throw e; // Re-thrown as-is
    }
    throw e; // Re-throws everything else (AuthenticationError)
  }
}

// BAD: Partially handles errors, re-throws others
/**
 * @throws {NetworkError}
 * @throws {AuthenticationError}
 */
export function partiallyProtectedProcessor(userId: string, token: string) {
  try {
    const user = fetchUserFromNetwork(userId);
    validateUserInput(user);
    const auth = authenticateUser(token);
    return { user, auth };
  } catch (e) {
    if (e instanceof ValidationError) {
      console.log("Handled validation error");
      return { error: "validation-handled" }; // Effectively caught
    }
    // NetworkError and AuthenticationError are re-thrown (not handled)
    throw e;
  }
}

// === PROPERLY DOCUMENTED FUNCTIONS (Should be suppressed) ===

/**
 * @throws {ValidationError} When user data is invalid
 * @throws {NetworkError} When network request fails
 * @throws {AuthenticationError} When authentication fails
 */
export function fullyDocumentedProcessor(userId: string, token: string) {
  const user = fetchUserFromNetwork(userId);
  validateUserInput(user);
  authenticateUser(token);
  return user;
}

/**
 * @throws {TypeError} When userId is not a string
 * @throws {ValidationError} When validation fails  
 * @throws {NetworkError} When network fails
 */
export function correctlyDocumentedWithCatch(userId: string) {
  if (typeof userId !== 'string') throw new TypeError("userId must be string");
  
  try {
    validateUserInput({ email: userId });
    fetchUserFromNetwork(userId);
    return "success";
  } catch (e) {
    if (e instanceof ValidationError) {
      throw e; // Re-thrown - must be documented
    } else if (e instanceof NetworkError) {
      throw e; // Re-thrown - must be documented  
    }
  }
}

// === CALL CHAIN PROPAGATION ===

// This should inherit throws from processUserUnsafely
/**
 * @throws {ValidationError}
 * @throws {NetworkError}
 * @throws {AuthenticationError}
 */
export function callerOfUnsafeProcessor(userId: string) {
  return processUserUnsafely(userId, "some-token");
}

// This should NOT inherit throws (effectively caught)
export function callerOfSafeProcessor(userId: string) {
  return processUserSafely(userId);
}

// This should inherit some throws (partially caught)
/**
 * @throws {NetworkError}
 * @throws {AuthenticationError}
 */
export function callerOfPartialProcessor(userId: string) {
  return partiallyProtectedProcessor(userId, "token");
}

/**
 * @throws {ValidationError} Inherited from called functions
 * @throws {NetworkError} Inherited from called functions
 * @throws {AuthenticationError} Inherited from called functions
 */
export function properlyDocumentedCaller(userId: string) {
  return fullyDocumentedProcessor(userId, "token");
}

// This should inherit throws from escape hatch function
export function callerOfEscapeHatchProcessor(userId: string) {
  // @it-throws
  return processUserWithEscapeHatch(userId, "token");
}

// === COMPLEX SCENARIOS ===

