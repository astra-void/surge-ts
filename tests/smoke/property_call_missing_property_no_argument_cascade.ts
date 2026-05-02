interface Store {
  value: string;
}

let store: Store = { value: "ok" };
store.missing(missing);