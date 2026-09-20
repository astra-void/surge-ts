declare function log(message: string): void;
declare function each(callback: (value: number) => void): void;

export const annotatedExpression = (): void => 1;
export const annotatedBlock = (): void => {
  return 1;
};
export const immediately = ((): void => "now")();
export const literal = {
  arrow: (): void => 1,
  method(): void {
    return 1;
  },
};

export class Holder {
  field = (): void => 1;
  method(): void {
    return 1;
  }
}

export function declaration(): void {
  return 1;
}

export const returnsVoidCall = (): void => log("fine");
export const returnsUndefined = (): void => undefined;
export const emptyBlock = (): void => {};
export const bareReturn = (): void => {
  return;
};
export const contextual: () => void = () => 1;
each((value) => value * 2);
each((value) => {
  return value;
});
