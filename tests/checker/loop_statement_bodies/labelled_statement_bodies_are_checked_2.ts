// A label only names a `break`/`continue` target; the statement it labels was
// being dropped along with it.
export function f() {
  lab: { const t: number = "x"; }
}
