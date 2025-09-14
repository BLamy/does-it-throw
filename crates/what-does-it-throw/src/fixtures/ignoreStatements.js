// @ts-nocheck
const someCondition = true
export class Something {
  someMethodThatThrows() { // should not be flagged
    // @it-throws
    throw new Error('hi khue') // should not be flagged
  }

  someMethodThatDoesNotThrow() {
    console.log('hi khue')
  }

  someMethodThatThrows2() { // should not be flagged
    if (someCondition) {
      // @it-throws
      throw new Error('hi khue') // should not be flagged
    }
  }

  nestedThrow() { // should not be flagged
    if (someCondition) {
      return true
    }
    // @it-throws
    throw new Error('hi khue') // should not be flagged
  }

  callNestedThrow() { // should not be flagged
    if (someCondition) {
      return true
    }
    if (someCondition) {
      return true
    }
    this.nestedThrow() // should not be flagged
  }
}

const _somethingCall = () => {
  const something = new Something()
  something.someMethodThatThrows()
}

export const somethingCall = () => {
  const something = new Something()
  something.someMethodThatThrows()
}


const someRandomThrow = () => { // should not be flagged
  // @it-throws
  throw new Error('some random throw') // should not be flagged
}

const someThrow = () => { // should not be flagged
  // @it-throws
  throw new Error('some throw') // should not be flagged
}

const server = http.createServer(async (req, res) => {
  switch (req.url) {
    case '/api/pong':
      console.log('pong!', INSTANCE_ID, PRIVATE_IP)
      // @it-throws
      throw new Error('') // should not be flagged
      break
    case '/api/ping':
      console.log('ping!', INSTANCE_ID, PRIVATE_IP)
      const ips = await someThrow()
      const others = ips.filter((ip) => ip !== PRIVATE_IP)

      others.forEach((ip) => {
        http.get(`http://[${ip}]:8080/api/pong`)
      })
      break
    case '/api/throw':
      someRandomThrow()
      break
  }

  res.end()
})

const wss = new WebSocketServer({ noServer: true })


function _somethingCall2() {
  const something = new Something()
  something.someMethodThatThrows()
}

export function somethingCall2() {
  const something = new Something()
  something.someMethodThatThrows()
}
