function take(value: string): void {}

interface Store {
  setState: (value: string) => void;
}

function setState(value: string): void {}

let store: Store = { setState };
take(missing1);
store.setState(missing2);
