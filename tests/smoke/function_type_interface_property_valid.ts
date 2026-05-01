interface Store {
  getState: () => string;
  setState: (value: string) => void;
}

function getState(): string {
  return "ok";
}

function setState(value: string): void {
}

let store: Store = {
  getState,
  setState,
};
