// tsc: a non-generic function takes no type arguments.
function makeString(): string { return "ok"; } let value: string = makeString<string>();
