function getState(): number {
  return 1;
}

function make(): () => string {
  return getState;
}
