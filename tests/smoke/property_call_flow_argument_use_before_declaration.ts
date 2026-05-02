interface Store {
  setState: (value: string) => void;
}

function setState(value: string): void {}

function read(store: Store): void {
  store.setState(missing);
  let missing = "ok";
}
