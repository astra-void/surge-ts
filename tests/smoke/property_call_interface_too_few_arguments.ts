interface Store {
  setState: (value: string, count: number) => void;
}

function setState(value: string, count: number): void {}

let store: Store = { setState };
store.setState("next");