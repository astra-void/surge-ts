type Fn = (() => Missing) | (() => string);

function a(): string {
  return "a";
}

let fn: Fn = true ? a : a;
