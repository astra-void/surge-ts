type Callback = () => string;

function getName(): string {
  return "Ada";
}

function make(): Callback {
  return getName;
}
