interface Store {
  getState: () => string;
}

function getState(): string {
  return "ok";
}

function take(value: string): void {}

let store: Store = { getState };
take(true ? store.getState() : missing);
