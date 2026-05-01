function a(): string {
  return "a";
}

function b(): number {
  return 1;
}

let fn: () => string = true ? a : b;
