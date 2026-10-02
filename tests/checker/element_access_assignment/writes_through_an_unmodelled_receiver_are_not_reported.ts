// A receiver whose element type surge cannot name stays unchecked rather than
// being held to a guess.
export function f(anything: any, key: string) {
  anything[key] = 1;
  const loose: unknown = {};
  (loose as any)[key] = "s";
}
