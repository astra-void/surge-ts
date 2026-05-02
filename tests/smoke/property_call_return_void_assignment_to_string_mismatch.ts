interface Store {
  log: () => void;
}

function log(): void {}

let store: Store = { log };
let value: string = store.log();
