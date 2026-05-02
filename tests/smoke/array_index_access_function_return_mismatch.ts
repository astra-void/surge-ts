function listener(): void {}

let listeners: (() => void)[] = [listener];

function read(): number {
  return listeners[0];
}
