function setState(value: string): void {
}

let api: { setState: (value: string) => void } = {
  setState,
  extra: 1,
};
