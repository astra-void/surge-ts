function listener(): void {}

let listeners: (() => void)[] = [listener];

function read(): () => void {
  return listeners[0];
}
