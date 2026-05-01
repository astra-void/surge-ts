type Fn = (callback: () => Missing) => string;

function f(callback: () => string): string {
  return callback();
}

let fn: Fn = f;
