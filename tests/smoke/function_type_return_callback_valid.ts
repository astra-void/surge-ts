function getState(): string {
  return "ok";
}

function make(): () => string {
  return getState;
}
