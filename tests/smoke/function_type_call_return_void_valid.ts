function noop(): void {
}

function apply(fn: () => void): void {
  return fn();
}
