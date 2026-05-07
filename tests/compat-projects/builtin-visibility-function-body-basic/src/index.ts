export function useBuiltins(value: string): number {
  const n = Number(value);
  if (isNaN(n)) return Date.now();
  return Math.floor(n);
}

export function useArrayFrom(values: Uint8Array): string {
  return Array.from(values).map((b: any) => b.toString(16)).join("");
}

export function useJson(value: unknown): string {
  return JSON.stringify(value);
}
