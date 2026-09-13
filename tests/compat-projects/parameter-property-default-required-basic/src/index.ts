export interface Encoder {
  encode(value: unknown): unknown;
}

const noopEncoder: Encoder = { encode: (value) => value };

export class Param {
  constructor(
    readonly value: unknown,
    readonly encoder: Encoder = noopEncoder,
    readonly label?: string,
  ) {}
}

export function encodeThrough(param: Param): unknown {
  return param.encoder.encode(param.value);
}

export function theOptionalOneStillNeedsAGuard(param: Param): number {
  return param.label.length;
}

export const omittingTheDefaultedArgumentIsStillFine = new Param(1);
