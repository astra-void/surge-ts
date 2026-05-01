interface Store {
  setState: (value: string) => void;
}

function setState(value: number): void {
}

let store: Store = {
  setState,
};
