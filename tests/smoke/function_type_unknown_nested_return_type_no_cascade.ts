type Fn = () => () => Missing;

function getName(): string {
  return "Ada";
}

function make(): () => string {
  return getName;
}

let fn: Fn = make;
