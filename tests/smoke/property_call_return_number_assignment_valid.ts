interface Store {
  getCount: () => number;
}

function getCount(): number {
  return 1;
}

let store: Store = { getCount };
let value: number = store.getCount();
