// @filename: a.ts
function getName(): string { return "Ada"; }
// @filename: b.ts
let first = getName(); let second: string = first;
