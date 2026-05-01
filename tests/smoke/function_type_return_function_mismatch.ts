function wrong(value: string): string {
  return "ok";
}

function make(): (value: string) => number {
  return wrong;
}
