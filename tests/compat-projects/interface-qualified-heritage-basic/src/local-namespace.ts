namespace Local {
  export interface Base {
    id: string;
  }

  export namespace Inner {
    export interface Nested {
      depth: number;
    }
  }
}

export interface LocalDerived extends Local.Base {
  extra: boolean;
}

export interface LocalNestedDerived extends Local.Inner.Nested {
  note: string;
}
