// @ts-nocheck

type SomeRandomType = {
  hiKhue: string
}

export function hiKhue({ hiKhue }: { hiKhue: string }) {
  throw new Error('hi khue')
}

export const someConstThatThrows = () => {
  throw new Error('hi khue')
}

const _ConstThatDoesNotThrow = ({
  someCondition
}: {
  someCondition: {
    hiKhue: string
  }
}) => {
  console.log('hi khue')
  someCondition.hiKhue
}

const _ConstThatThrows = () => {
  throw new Error('hi khue')
}

const callToConstThatThrows = () => {
  someConstThatThrows()
}  // BUG: flagged when it should not be


export const someConstThatThrows2 = () => {
  if (someCondition) {
    throw new Error('hi khue')
  }
}

export const callToConstThatThrows2 = () => {
  someConstThatThrows2()
}     // BUG: flagged when it should not be


export function callToConstThatThrows3() {
  const hello: SomeRandomType = {  // BUG: flagged when it should not be
    hiKhue: 'hi khue'  // BUG: flagged when it should not be
  }  // BUG: flagged when it should not be

  someConstThatThrows2()
}  // BUG: flagged when it should not be


function callToConstThatThrows4() {
  someConstThatThrows2()
}  // BUG: flagged when it should not be

