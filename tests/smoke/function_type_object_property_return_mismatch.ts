function getState(): number {
  return 1;
}

let api: { getState: () => string } = {
  getState,
};
