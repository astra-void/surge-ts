declare const text: string;
declare const flag: Promise<boolean>;
declare const pending: Promise<string>;
declare function takesPromise(value: Promise<string>): void;
declare function takesFlags(first: boolean, second: boolean): void;

export const wrapped: Promise<string> = text;
export const unwrapped: string = pending;
export const other: Promise<number> = pending;
takesPromise(text);
takesPromise(pending);

export function plain(): Promise<string> {
  return text;
}

export async function returnsValue(): Promise<string> {
  return text;
}

export async function returnsPromise(): Promise<string> {
  return pending;
}

export async function returnsWrongValue(): Promise<string> {
  return 1;
}

export async function returnsWrongPromise(): Promise<string> {
  return flag;
}

export const arrowValue = async (): Promise<string> => text;
export const arrowWrong = async (): Promise<string> => 1;

export class Service {
  async count(): Promise<number> {
    return 0;
  }
  async broken(): Promise<number> {
    return text;
  }
}

export async function awaitedArgument(): Promise<void> {
  takesFlags(await flag, true);
  takesFlags(flag, true);
}
