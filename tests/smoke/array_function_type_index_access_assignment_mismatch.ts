function listener(): void {}

let listeners: (() => void)[] = [listener];
let first: string = listeners[0];
