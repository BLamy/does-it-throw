export const someObjectLiteral = {
  objectLiteralThrow({ someArg }: { someArg: string }) { // should be flagged
    throw new Error('hi khue') // should be flagged
  },
  nestedObjectLiteral: {
    nestedObjectLiteralThrow: () => { // should be flagged
      throw new Error('hi khue') // should be flagged
    }
  }
}

export const SomeObject = {
  someExampleThrow: () => { // should be flagged
    throw new Error('hi khue') // should be flagged
  }
}

export function callToLiteral() { // BUG: not flagged when it should be
  someObjectLiteral.objectLiteralThrow({ someArg: 'hi' }) // should be flagged
}

export const callToLiteral2 = () => { // BUG: not flagged when it should be
  someObjectLiteral.objectLiteralThrow({ someArg: 'hi' }) // should be flagged
}

export const callToLiteral3 = () => { // should be flagged
  someObjectLiteral.nestedObjectLiteral.nestedObjectLiteralThrow() // should be flagged
  SomeObject.someExampleThrow() // should be flagged
} // BUG should not be flagged
