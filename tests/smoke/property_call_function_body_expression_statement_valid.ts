interface Store {
  setState: (value: string) => void;
}

function setState(value: string): void {}

function update(store: Store): void {
  store.setState("next");
}