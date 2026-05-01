function a(): string {
  return "a";
}

let fn: (() => Missing) | (() => string) = true ? a : a;
