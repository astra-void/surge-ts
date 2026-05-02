function listener(): void {}

let pair: [() => void, string] = [listener, "ready"];
let fn: () => string = pair[0];
