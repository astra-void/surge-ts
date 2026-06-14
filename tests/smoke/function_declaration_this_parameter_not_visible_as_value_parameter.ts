interface Context {
  id: number;
}

function run(this: Context, value: string): void {
  const label: string = value;
}
