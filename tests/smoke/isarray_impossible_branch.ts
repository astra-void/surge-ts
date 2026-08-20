type Kind = "object" | "array" | "string";

export function expand(kind: Kind | undefined): string[] {
  if (Array.isArray(kind)) {
    return kind.map((k) => String(k));
  }
  return [String(kind)];
}
