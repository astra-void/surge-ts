export function optionalWithDefault(value?: number = 1): void {
  void value;
}

export function requiredAfterOptional(first?: number, second: string): void {
  void first;
  void second;
}

export const arrowRequiredAfterOptional = (first?: number, second: string): void => {
  void first;
  void second;
};

export declare function signatureDefault(value: number = 1): void;

export class Signatures {
  constructor(public value: number);
  constructor(value: number) {
    void value;
  }
}

// Defaulted parameters do not make the next parameter illegal, and a rest
// parameter after an optional one is fine.
export function defaultedThenRequired(first = 1, second: string): void {
  void first;
  void second;
}

export function optionalThenRest(first?: number, ...rest: string[]): void {
  void first;
  void rest;
}

export class Implementation {
  constructor(public value: number = 1) {}
}
