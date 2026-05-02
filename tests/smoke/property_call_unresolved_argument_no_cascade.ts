interface Store {
  setState: (value: string) => void;
}

function setState(value: string): void {}

let store: Store = { setState };
store.setState(missing);