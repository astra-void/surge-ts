let store: { getState?: () => string } = {};
let fn: (() => string) | undefined = store.getState;
