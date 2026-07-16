export async function emptyBody(): Promise<void> {}

export async function bodyWithoutReturn(): Promise<void> {
  const pending = 1;
  void pending;
}

export async function bareReturn(): Promise<void> {
  return;
}

export async function undefinedPromise(): Promise<undefined> {}

export async function anyPromise(): Promise<any> {}

type Done = Promise<void>;
export async function aliasedPromiseVoid(): Done {}

type MyVoid = void;
export function aliasedVoid(): MyVoid {}

export const arrowVoid = async (): Promise<void> => {};

export const handlers = {
  async flush(): Promise<void> {},
};

export class Worker {
  async run(): Promise<void> {}

  async stop(): Promise<void> {
    const pending = false;
    void pending;
  }
}

export async function valueReturn(): Promise<string> {
  return "ok";
}

// Intentional TS2355: a value-promise async function must still return a value.
export async function missingValue(): Promise<string> {}
