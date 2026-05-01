type Callback = () => string;

function use(callback: Callback): string {
  return callback();
}

function getName(): string {
  return "Ada";
}

let result: string = use(getName);
