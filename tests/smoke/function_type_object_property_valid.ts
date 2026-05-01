function getState(): string {
  return "ok";
}

let api: { getState: () => string } = {
  getState,
};
