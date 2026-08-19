export function f(s: string, needle: string, position: number | undefined): boolean {
  return (
    s.includes(needle, position) ||
    s.startsWith(needle, position) ||
    s.endsWith(needle, position) ||
    s.indexOf(needle, position) >= 0 ||
    s.lastIndexOf(needle, position) >= 0
  );
}
