function listener(): void {}

let pair: [() => void, string] = [listener, "ready"];
let fn: () => void = pair[0];
