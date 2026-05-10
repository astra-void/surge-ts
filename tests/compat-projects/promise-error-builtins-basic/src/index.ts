export async function allValues(values: Promise<string>[]): Promise<string[]> {
  return await Promise.all(values);
}

export function throwNew(): never {
  throw new Error("bad");
}

export function throwCall(): never {
  throw Error("bad");
}

export const e1 = new Error("bad");
export const e2 = Error("bad");
