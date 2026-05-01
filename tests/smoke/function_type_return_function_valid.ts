function length(value: string): number {
  return 1;
}

function make(): (value: string) => number {
  return length;
}
