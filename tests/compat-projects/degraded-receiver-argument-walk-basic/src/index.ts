export {}
declare const opaque: unknown;

opaque?.["method"]((value: number) => {
  unresolvedInsideCallback;
  return value;
});

declare const maybeCallable: unknown;
maybeCallable?.(() => {
  alsoUnresolved;
});

export function throughTypeParameter<T>(receiver: T) {
  receiver?.["each"](() => {
    unresolvedUnderTypeParameter;
  });
}

declare const known: { each(cb: () => void): void } | undefined;
known?.each(() => {
  unresolvedUnderKnownReceiver;
});
