type Fn = (value: Missing) => string;

function f(value: string): string {
  return value;
}

let fn: Fn = f;
