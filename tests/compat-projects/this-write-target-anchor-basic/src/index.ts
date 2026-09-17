class Settings {
  count: number = 0;
  shape: { a: number } = { a: 1 };

  constructor() {
    this.count = "zero";
  }

  update() {
    this.shape = { a: "one" };
    this.shape = {};
    this.shape = { a: 2 };
  }
}

interface Holder {
  value: string;
}

function assign(this: Holder) {
  this.value = 1;
}
