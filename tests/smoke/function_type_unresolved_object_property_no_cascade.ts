function getState(): string {
  return "ok";
}

let store: { getState: () => Missing } = {
  getState,
};
