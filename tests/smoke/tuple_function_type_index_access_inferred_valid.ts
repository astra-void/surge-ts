function listener(): void {}

let pair: [() => void, string] = [listener, "ready"];
let fn = pair[0];
let exact: () => void = fn;
