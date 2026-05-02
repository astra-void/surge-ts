interface Store {
  getValue: () => string | number;
}

function getValue(): string | number {
  return 1;
}

let store: Store = { getValue };
let value: string | number = true ? store.getValue() : 1;
