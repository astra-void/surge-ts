function listener(value: string): void {}

let listeners: ((value: string) => void)[] = [listener];
let first = listeners[0];
let value: () => void = first;
