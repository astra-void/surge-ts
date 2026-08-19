type Merged =
  | { valid: true; data: unknown }
  | { valid: false; errorPath: (string | number)[] };

declare function merge(a: unknown, b: unknown): Merged;

export function run(a: unknown, b: unknown): unknown {
  const merged = merge(a, b);
  if (!merged.valid) {
    throw new Error(JSON.stringify(merged.errorPath));
  }
  return merged.data;
}
