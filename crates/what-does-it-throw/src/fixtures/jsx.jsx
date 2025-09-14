export const someThrow = () => { // should be flagged
  throw new Error('some error') // should be flagged
}
export function someThrow2() { // should be flagged
  throw new Error('some error') // should be flagged
}

export const someTsx = () => { // should be flagged
  if (something) { // should NOT be flagged
    throw new Error() // should be flagged
  } // should NOT be flagged
  return <div>some tsx</div> // should NOT be flagged
}

export async function someAsyncTsx() { // should be flagged
  if (something) { // should NOT be flagged
    throw new Error() // should be flagged
  } // should NOT be flagged
  return <div>some tsx</div> // should NOT be flagged
}

export async function callToThrow() { // should be flagged
  someThrow() // should be flagged
  someThrow2() // should be flagged
  return <div>some tsx</div> // BUG should not be flagged
} // BUG should not be flagged

export const someTsxWithJsx = async () => {
  someThrow() // should be flagged
  someThrow2() // should be flagged
  return <div>some tsx</div> // BUG should not be flagged
} // BUG should not be flagged
