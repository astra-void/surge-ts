interface Store {
  setState: (value: string) => void;
}

function setState(value: string): void {}

function read(store: Store): void {
  let missing: string;
  store.setState(missing);
}
