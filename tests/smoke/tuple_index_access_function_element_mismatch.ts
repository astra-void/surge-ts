function listener(value: string): void {}

let pair: [(value: string) => void, string] = [listener, "ready"];
let fn: () => void = pair[0];
