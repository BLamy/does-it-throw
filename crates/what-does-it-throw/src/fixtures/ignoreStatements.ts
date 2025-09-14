import http from 'http'

const someCondition = true

class NestedError extends Error {
  constructor(message: string) {
    super(message)
  }
}

export class Something {
  someMethodThatSupressThrows() { // should not be flagged
    // @it-throws
    throw new Error('hi khue') // should not be flagged
  }

  someMethodThatDoesNotThrow() {
    console.log('hi khue')
  }

  someMethodThatThrows2() { // should be flagged
    if (someCondition) {
      // @some-random-ignore
      throw new Error('hi khue') // should be flagged
    }
  }

  /**
   * @throws {NestedError} - description
   */
  nestedThrow() { // should not be flagged
    if (someCondition) {
      return true
    }
    throw new NestedError('hi khue') // should not be flagged
  }

  /**
   * @throws {NestedError} - description
   */
  callNestedThrow() {
    if (someCondition) {
      return true
    }
    if (someCondition) {
      return true
    }
    this.nestedThrow() // this should not be flagged because it is marked by the parent function
  }
}

export const somethingCall = () => {
  const something = new Something()
  something.someMethodThatSupressThrows()
}


const someRandomSuppressedThrow = () => {
  // @it-throws
  throw new Error('some random throw')
}

const server = http.createServer(async (req, res) => {
  switch (req.url) {
    case '/api/pong':
      console.log('pong')
      // @it-throws
      throw new Error('') // should not be flagged
      break
    case '/api/ping':
      console.log('ping')
      const ips = await someRandomSuppressedThrow() // should not be flagged
      break
    case '/api/throw':
      someRandomSuppressedThrow()
      break
  }

  res.end()
})

/**
 * @throws {NestedError}
 */
export function somethingCall2() {
  const something = new Something()
  something.someMethodThatSupressThrows() // should not be flagged

  something.someMethodThatThrows2() // should be flagged
  something.nestedThrow() // should not be flagged
}
