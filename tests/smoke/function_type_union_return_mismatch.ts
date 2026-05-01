function f(value: string): string {
  return value;
}

let mapper: (value: string) => "idle" | "done" = f;
