export function check(value?: string): string | null {
  if (!value) return null;

  const parts = value.split(":");
  if (parts.length !== 3) return null;

  const first = parts[0];
  const second = parts[1];
  const result = `${first}:${second}`;
  return result;
}
