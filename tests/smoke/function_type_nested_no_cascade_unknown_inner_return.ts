type Callback = () => Missing;

function getName(): string {
  return "Ada";
}

function make(): Callback {
  return getName;
}
