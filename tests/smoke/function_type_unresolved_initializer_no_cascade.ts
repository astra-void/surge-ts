type Fn = (value: Missing) => string;

function getState(): string {
  return "ok";
}

let fn: Fn = getState;
