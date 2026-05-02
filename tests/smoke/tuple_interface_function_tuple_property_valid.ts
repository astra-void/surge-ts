interface Store {
  pair: [() => void, string];
}

function listener(): void {}

let store: Store = { pair: [listener, "ready"] };
