interface Store {
  setState: (value: string) => void;
}

function setState(value: string): void {}

let store: Store = { setState };

function update(): void {
  store.setState("next");
}