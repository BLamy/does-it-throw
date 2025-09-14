export const SomeThrow = () => { // should be flagged
    throw new Error('never gonna let you down'); // should be flagged
}

export function Something () { // should be flagged
  throw new Error('never gonna run around and desert you') // should be flagged
  return [] as string[]
}

export function NotAnError() { // should not be flagged
  return 'not an error' // should not be flagged
}

export const someObjectLiteral = {
  objectLiteralThrow: () => { // should be flagged
    throw new Error('hi khue') // should be flagged
  }
}