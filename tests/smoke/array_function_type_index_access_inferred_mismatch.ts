function listener(): void {}

let listeners: (() => void)[] = [listener];
let first = listeners[0];
let value: (value: string) => void = first;
