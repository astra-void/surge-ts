// @filename: box.ts
type Box<T> = { value: T };
// @filename: index.ts
function getValue(): string { return "ok"; } let box: Box<() => string> = { value: getValue };
