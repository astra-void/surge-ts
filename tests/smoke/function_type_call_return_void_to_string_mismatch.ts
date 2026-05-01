function noop(): void {
}

function apply(fn: () => void): string {
  return fn();
}
