function getName(): string {
  return "Ada";
}

function make(): () => string {
  return getName;
}
