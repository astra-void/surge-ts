function a(): string {
  return "a";
}

function b(): string {
  return "b";
}

let fn: () => string = true ? a : b;
