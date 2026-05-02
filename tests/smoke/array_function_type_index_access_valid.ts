function listener(): void {}

let listeners: (() => void)[] = [listener];
let first: () => void = listeners[0];
