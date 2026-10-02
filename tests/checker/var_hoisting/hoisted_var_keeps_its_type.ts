// The hoisted binding keeps the type it was declared with, so a misuse after the
// block still reports.
export function f(): string {
{
var hoisted = 1;
}
return hoisted;
}
