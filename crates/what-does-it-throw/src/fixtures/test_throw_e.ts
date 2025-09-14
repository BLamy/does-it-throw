// Test case for throw e analysis
class ValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ValidationError';
  }
}

class NetworkError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'NetworkError';
  }
}

class AuthenticationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'AuthenticationError';
  }
}

class DatabaseError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'DatabaseError';
  }
}

/**
 * @throws {ValidationError} Validation errors
 */
function validateUserInput(data: any) {
  if (!data) throw new ValidationError("Data is required");
  if (!data.email) throw new ValidationError("Email is required");
}

/**
 * @throws {NetworkError} Network errors
 */
function fetchUserFromNetwork(userId: string) {
  if (!userId) throw new NetworkError("User ID is required");
  if (Math.random() > 0.7) throw new NetworkError("Connection timeout");
  return { id: userId, email: "user@example.com" };
}

/**
 * @throws {AuthenticationError} Authentication errors
 */
function authenticateUser(token: string) {
  if (!token) throw new AuthenticationError("Token is required");
  if (token === "invalid") throw new AuthenticationError("Invalid token");
  return { valid: true, user: "authenticated" };
}

/**
 * @throws {DatabaseError} Database errors
 */
function saveToDatabase(data: any) {
  if (!data) throw new DatabaseError("No data to save");
  if (Math.random() > 0.8) throw new DatabaseError("Database connection failed");
  return { saved: true, id: "12345" };
}

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
      throw e; // Re-thrown as-is - should be analyzed as AuthenticationError
    }
    throw e; // DatabaseError falls through and is re-thrown - should be analyzed as DatabaseError
  }
} 