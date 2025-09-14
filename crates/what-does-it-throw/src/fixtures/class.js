
const someCondition = true
export class Something {
  constructor() { // should be flagged
    throw new Error('hi khue') // should be flagged
  }

  someMethodThatThrows() { // should be flagged
    throw new Error('hi khue') // should be flagged
  }

  someMethodThatDoesNotThrow() {
    console.log('hi khue')
  }

  someMethodThatThrows2() { // should be flagged
    if (someCondition) {
      throw new Error('hi khue') // should be flagged
    }
  }

  nestedThrow() { // should be flagged
    if (someCondition) {
      return true
    }
    throw new Error('hi khue') // should be flagged
  }

  callNestedThrow() { // should be flagged
    if (someCondition) {
      return true
    }
    if (someCondition) {
      return true
    }
    this.nestedThrow() // should be flagged
  }
}

const _somethingCall = () => {
  const something = new Something()
  something.someMethodThatThrows() // should be flagged
}

export const somethingCall = () => {
  const something = new Something()
  something.someMethodThatThrows() // should be flagged
}

function _somethingCall2() {
  const something = new Something()
  something.someMethodThatThrows() // should be flagged
}

export function somethingCall2() {
  const something = new Something()
  something.someMethodThatThrows() // should be flagged
}
