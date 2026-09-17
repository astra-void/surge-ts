declare const keepGoing: boolean;

function forLoop() {
  let value: string | number = "start";
  for (let i = 0; i < 2; i++) {
    value = 1;
  }
  const text: string = value;
}

function whileLoop() {
  let value: string | number = "start";
  while (keepGoing) {
    value = 1;
  }
  const text: string = value;
}

function doWhileLoop() {
  let value: string | number = "start";
  do {
    value = 1;
  } while (keepGoing);
  const text: string = value;
  const count: number = value;
}

function forOfLoop(items: number[]) {
  let value: string | number = "start";
  for (const item of items) {
    value = item;
  }
  const text: string = value;
}

function forInLoop(source: object) {
  let value: string | number = "start";
  for (const key in source) {
    value = key.length;
  }
  const text: string = value;
}

function untouched() {
  let value: string | number = "start";
  while (keepGoing) {
    const local = 1;
  }
  const text: string = value;
}
