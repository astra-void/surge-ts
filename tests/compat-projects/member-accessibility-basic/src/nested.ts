export class Outer {
  private secret = 1;

  make() {
    const outer = this;
    return class Inner {
      read(): number {
        return outer.secret;
      }
    };
  }
}

export class Unrelated {
  look(outer: Outer): number {
    return outer.secret;
  }
}
