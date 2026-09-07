export function storeResolver(): (() => void) | null {
  let handler: (() => void) | null = null;
  const promise = new Promise<void>((resolve) => {
    handler = resolve;
  });
  void promise;
  return handler;
}

declare const acceptsVoid: (value: void) => void;
declare const acceptsVoidUnion: (value: void | PromiseLike<void>) => void;
declare const acceptsString: (value: string) => void;
declare const voidThenRequired: (a: void, b: string) => void;

export const plainVoid: () => void = acceptsVoid;
export const voidUnion: () => void = acceptsVoidUnion;
export const stringParameter: () => void = acceptsString;
export const trailingRequired: () => void = voidThenRequired;

export function callWithoutTheVoidArgument(): void {
  acceptsVoid();
  acceptsVoidUnion();
}
