const someThrow = (something: boolean) => { // should be flagged
  if (something) {
    while (true) {
      throw new Error("oh no"); // should be flagged
    }
  } else {
    for (let i = 0; i < 10; i++) {
      throw new Error("oh no"); // should be flagged
    }
  }
}
class Test {
  badMethod() { // shoudl be flagged
    throw new Error("oh no"); // shoudl be flagged
  }
}

const callToSomeThrow = () => {
  const testMethod = new Test();
  return {
    test: someThrow(true), // should be flagged
    testing: () => someThrow(true), // should be flagged
    array: [someThrow(true), someThrow(false)], // should be flagged
    object: { test: someThrow(true) }, // should be flagged
    class: testMethod.badMethod(), // should be flagged
  }
}

function test() { // BUG: not flagged when it should be
  return someThrow(true); // should be flagged
}