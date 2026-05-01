function getName(): string {
  return "Ada";
}

let store: { callback: () => string } = {
  callback: getName,
};
