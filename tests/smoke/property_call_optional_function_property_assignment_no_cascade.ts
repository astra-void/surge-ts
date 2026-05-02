let store: { getState?: () => string } = {};
let value: string = store.getState(missing);
