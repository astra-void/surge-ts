interface Store {
  listeners: (() => number)[];
}

function listener(): string {
  return "ok";
}

let store: Store = { listeners: [listener] };
