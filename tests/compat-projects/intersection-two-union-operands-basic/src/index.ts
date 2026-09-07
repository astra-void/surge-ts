interface QueryClient {
  clear(): void;
}
interface QueryClientConfig {
  retries: number;
}

type ClientConfig =
  | { queryClient?: QueryClient; queryClientConfig?: never }
  | { queryClientConfig?: QueryClientConfig; queryClient?: never };

interface HelpersExternal {
  client: string;
}
interface HelpersInternal {
  router: string;
  ctx: number;
}

type HelperOptions = ClientConfig & (HelpersExternal | HelpersInternal);

export function describeHelpers(opts: HelperOptions): string {
  if ('router' in opts) {
    const { ctx, router } = opts;
    return `${router}:${ctx}`;
  }
  const { client } = opts;
  return client;
}

export function bothUnionsStayReadable(opts: HelperOptions): number {
  if ('router' in opts) {
    return opts.queryClientConfig?.retries ?? opts.ctx;
  }
  opts.queryClient?.clear();
  return opts.client.length;
}

export function theSecondUnionIsStillTyped(opts: HelperOptions): number {
  if (!('router' in opts)) {
    return 0;
  }
  return opts.ctx;
}

export function elementTypesAreReal(opts: HelperOptions): void {
  if ('router' in opts) {
    const wrong: number = opts.router;
    void wrong;
  }
}
