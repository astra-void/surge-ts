type Callback = () => string;

function use(callback: Callback): string {
  return callback();
}

function getCount(): number {
  return 1;
}

let result: string = use(getCount);
