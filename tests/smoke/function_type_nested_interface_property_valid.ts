interface Store {
  callback: () => string;
}

function getName(): string {
  return "Ada";
}

let store: Store = {
  callback: getName,
};
