interface Counter {
  count: number;
  label: string;
}

function read(this: Counter) {
  const text: number = this.label;
  return this.missing;
}

function write(this: Counter) {
  this.count = 1;
  this.extra = 2;
}

function lengthOf(this: Counter, offset: number): number {
  return this.label.length + offset;
}

function inline(this: { q: number }) {
  const q: string = this.q;
}
