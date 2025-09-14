// @ts-nocheck

export const someConstThatThrows = () => { // should be flagged
  throw new Error('hi khue') // should be flagged
}

function callToConstThatThrows4() {
  someConstThatThrows() // should be flagged
} // BUG: Flagged when it should not be
