export function omitA({ a, ...rest }: { a: number; b: number }) {
  return rest;
}
