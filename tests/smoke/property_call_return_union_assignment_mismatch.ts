interface Store {
  getStatus: () => "idle" | "done";
}

function getStatus(): "idle" | "done" {
  return "idle";
}

let store: Store = { getStatus };
let value: boolean = store.getStatus();
