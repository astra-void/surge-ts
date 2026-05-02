interface Store {
  update: (value: string, count: number) => void;
}

function update(value: string, count: number): void {}

let store: Store = { update };
store.update("next", 1);