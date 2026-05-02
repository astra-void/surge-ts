interface Store {
  listeners: (() => void)[];
}

function listener(): string {
  return "ok";
}

let store: Store = { listeners: [listener] };
