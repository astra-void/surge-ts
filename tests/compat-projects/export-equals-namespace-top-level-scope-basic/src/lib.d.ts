type NativeThing = Thing;
export = NS;
export as namespace NS;
declare namespace NS {
  interface Thing {
    fromNamespace: true;
  }
  interface Holder {
    native: NativeThing;
    own: Thing;
  }
}
