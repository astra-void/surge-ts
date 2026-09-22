export type Fetch = typeof globalThis.fetch;

export interface Options {
  fetch?: Fetch;
}

export function pickFetch(options: Options): void {
  const { fetch = globalThis.fetch } = options;
  const mismatched: number = fetch;
  void mismatched;
}

interface Ctx {
  response?: Response;
}

declare function doFetch(request: string): Promise<Response>;

export async function assigned(ctx: Ctx, request: string): Promise<string> {
  ctx.response = await doFetch(request);
  return ctx.response.text();
}

export function assignedInCallback(ctx: Ctx, request: string): Promise<string> {
  return doFetch(request).then((response) => {
    ctx.response = response;
    return ctx.response.text();
  });
}
