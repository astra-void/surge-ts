export namespace Outer {
  export interface Member {
    value: string;
  }

  export namespace Inner {
    export interface Leaf {
      leaf: number;
    }
  }
}
