// A `Promise<void>` executor may call `resolve()` with no argument: the
// contextual return type pins `T = void`, and a `void` parameter is optional.
export async function delayVoid(): Promise<void> {
  return new Promise((resolve, reject) => {
    resolve();
    reject(new Error("x"));
  });
}

export function explicitVoid() {
  return new Promise<void>((resolve) => resolve());
}

export function makeNumber(): Promise<number> {
  return new Promise((resolve) => resolve(5));
}
