function read(store: { getState?: () => string }): string {
  return store.getState(missing);
}
