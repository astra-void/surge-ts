interface Store {
  getStatus: () => "idle";
}

function getStatus(): "idle" {
  return "idle";
}

let store: Store = { getStatus };
let value: "done" = store.getStatus();
