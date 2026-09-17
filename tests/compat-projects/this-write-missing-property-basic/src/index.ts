class Widget {
  size = 1;

  constructor() {
    this.undeclared = 1;
  }

  resize() {
    this.size = 2;
    this.width = 3;
  }

  static configure() {
    this.defaults = {};
  }
}

class Resized extends Widget {
  grow() {
    this.size = 5;
    this.height = 1;
  }
}

class Bag {
  [key: string]: unknown;
  fill() {
    this.anything = 1;
  }
}
new Bag().other = 2;
const bagValue: unknown = new Bag().other;

class Table {
  [row: number]: string;
}
declare const table: Table;
const cell: number = table[0];

declare class SymbolKeyed {
  [key: symbol]: number;
}
declare const symbolKeyed: SymbolKeyed;
symbolKeyed.named = 1;
