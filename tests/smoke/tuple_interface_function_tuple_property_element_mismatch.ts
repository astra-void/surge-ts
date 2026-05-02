interface Store {
  pair: [() => void, string];
}

function listener(value: string): void {}

let store: Store = { pair: [listener, "ready"] };
