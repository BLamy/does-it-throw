import { SomeThrow, SomeThrow as SomeThrow2, NotAnError } from './something'

export function test() {
  try {
    SomeThrow() // should be flagged
  } catch (e) {
    console.log(e)
  }
  try {
    SomeThrow2() // BUG: not flagged when it should be
  } catch (e) {
    console.log(e)
  }
  try {
    NotAnError() // should not be flagged
  } catch (e) {
    console.log(e)
  }
}
