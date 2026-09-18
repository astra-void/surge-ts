namespace Outer {
  export namespace Inner {
    export const a = 1;
  }
  export namespace Inner {
    export const b = 2;
  }
}

export const total = Outer.Inner.a + Outer.Inner.b;
