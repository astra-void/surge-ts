let text: string = "";
let count: number = 0;
declare const pair: [number, string];

[text] = [1];
[text, count] = ["a", "b"];
[count, text] = pair;
[text, count] = pair;
({ a: text } = { a: [] as number[] });

function local() {
  let inner = "";
  [inner] = [2];
}

const fixed = 1;
[fixed] = [2];
[notDeclared] = [1];

let left = 1;
let right = 2;
[left, right] = [right, left];

let maybe: string | undefined;
[maybe = "default"] = [];
