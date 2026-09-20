export class Gone extends Missing {}

export class GoneGeneric<T> extends AlsoMissing {
  value!: T;
}

export interface Shape extends NotAType {
  sides: number;
}
