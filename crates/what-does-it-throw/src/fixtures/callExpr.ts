const connection = {
  onInitialized: (fn: (hasConfigurationCapability: boolean, hasWorkspaceFolderCapability: boolean) => void) => {
    fn(true, true)
  },
  onInitialized2: (fn: () => void) => {
    fn()
  },
  client: {
    register: () => {},
  },
  workspace: {
    onDidChangeWorkspaceFolders: (fn: (_event: any) => void) => {
      fn({})
    },
  },
  oneWithASecondArg: (obj: any, fn: () => void) => {
    fn()
  },
}

const SomeRandomCall = (fn: () => void) => {
  fn()
}

const SomeRandomCall2 = (fn: () => void) => {
  fn()
}

const SomeThrow = () => { // should be flagged
  throw new Error('hi khue') // should be flagged
}

function SomeThrow2() { // should be flagged
  throw new Error('hi khue') // should be flagged
}

connection.onInitialized((hasConfigurationCapability: boolean, hasWorkspaceFolderCapability: boolean) => {
  if (hasConfigurationCapability) {
    // Register for all configuration changes.
    connection.client.register()
  }
  if (hasWorkspaceFolderCapability) {
    connection.workspace.onDidChangeWorkspaceFolders((_event) => {
      console.log(`Workspace folder change event received. ${JSON.stringify(_event)}`)
    })
  }
  SomeThrow() // should be flagged
  SomeThrow2() // should be flagged
})

connection.onInitialized2(() => { // should be flagged
  throw new Error('hi khue') // should be flagged
})

SomeRandomCall(() => { // should be flagged
  throw new Error('hi khue') // should be flagged
})

SomeRandomCall2(() => { // not Flagged but should be
  SomeThrow() // should be flagged
  SomeThrow2() // should be flagged
})

connection.oneWithASecondArg({}, () => { // should be flagged
  throw new Error('hi khue') // should be flagged
})

const testGetter = {
  get test() { // BUG: not flagged but should be flagged
    SomeThrow() // should be flagged
    return true
  }
}

const array = [
  SomeThrow(), // should be flagged
  SomeThrow2(), // should be flagged
]
