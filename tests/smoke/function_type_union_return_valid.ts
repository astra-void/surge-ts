function f(value: string): "idle" | "done" {
  return "idle";
}

let mapper: (value: string) => string = f;
