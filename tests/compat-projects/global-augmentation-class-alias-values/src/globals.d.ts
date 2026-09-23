export {};

declare namespace Local.Inner {
  export let value: number;
}

declare global {
  class GlobalWidget {
    size: number;
    constructor(size: number);
  }
  export import InnerValue = Local.Inner.value;
  interface OnlyAType {
    kind: string;
  }
}
