interface Store {
  listeners: (() => void)[];
}

function listener(): void {}

let store: Store = { listeners: [listener] };
