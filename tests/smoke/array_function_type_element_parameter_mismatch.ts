function listener(value: number): void {}

let listeners: ((value: string) => void)[] = [listener];
