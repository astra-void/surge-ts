export class Point {
  readonly x = 1;
  readonly y: number;

  constructor(readonly label: string) {
    this.x = 1;
    this.y = 2;
    this.label = "origin";
    const later = () => {
      this.y = 3;
    };
    later();
  }

  move() {
    this.x = 2;
    this.y = 5;
    this.label = "moved";
  }

  get doubled() {
    this.y = 7;
    return this.y * 2;
  }
}

export class Labelled extends Point {
  readonly own: string;
  mutable = 1;

  constructor() {
    super("derived");
    this.y = 8;
    this.own = "a";
  }

  rename() {
    this.own = "b";
    this.mutable = 2;
    this.mutable = "wide";
  }
}
