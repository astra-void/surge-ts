function setState(value: string): void {}

let store: { setState: (value: string) => void } = { setState };
store.setState("next");