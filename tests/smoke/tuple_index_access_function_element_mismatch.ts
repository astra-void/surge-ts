function listener(): void {}

let pair: [() => void, string] = [listener, "ready"];
let fn: (value: string) => void = pair[0];
