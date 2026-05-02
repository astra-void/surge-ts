interface Store {
  setState: (value: string) => void;
}

function setState(value: string): void {}

let store: Store = { setState };
let target: { value: string } = { value: store.setState(missing) };