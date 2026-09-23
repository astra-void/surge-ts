class Other {
  x = 1;
}

export class Host {
  y = 2;
  explicit(this: Other): number {
    return this.x;
  }
  implicit(): number {
    return this.y;
  }
  static make(this: typeof Other): Other {
    return new this();
  }
  wrong(this: Other): number {
    return this.y;
  }
}
