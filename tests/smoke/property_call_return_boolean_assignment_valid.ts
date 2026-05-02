interface Store {
  isReady: () => boolean;
}

function isReady(): boolean {
  return true;
}

let store: Store = { isReady };
let value: boolean = store.isReady();
