type Fn = () => Missing;

function f(): string {
  return "ok";
}

let fn: Fn = f;
