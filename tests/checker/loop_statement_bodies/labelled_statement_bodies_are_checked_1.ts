// A label only names a `break`/`continue` target; the statement it labels was
// being dropped along with it.
export function f() {
  outer: for (const row of [[1]]) { const s: string = row; }
}
