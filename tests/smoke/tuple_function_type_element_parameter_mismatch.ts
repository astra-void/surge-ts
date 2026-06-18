function listener(value: number): void {}

let pair: [(value: string) => void, string] = [listener, "ready"];
