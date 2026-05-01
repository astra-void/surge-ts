type Callback = (value: Missing) => string;

function f(value: string): string {
  return value;
}

let fn: Callback = f;
