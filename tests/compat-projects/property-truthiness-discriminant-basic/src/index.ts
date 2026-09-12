type Base<T> = { config: (t: T) => string };
type PrepassHelper = (opts: { parent: SsrOptions<any> }) => void;
type SsrOptions<T> = Base<T> & {
  ssr: true | ((opts: { ctx: number }) => boolean | Promise<boolean>);
  ssrPrepass: PrepassHelper;
};
type NoSsrOptions<T> = Base<T> & { ssr?: false };

export function withSsr<T>(opts: NoSsrOptions<T> | SsrOptions<T>): string {
  if (opts.ssr) {
    opts.ssrPrepass({ parent: opts });
    return 'ssr';
  }
  const off: NoSsrOptions<T> = opts;
  return off.config(null as any);
}

interface WithHandler {
  handler: (x: number) => void;
  name: 'handled';
}
interface WithoutHandler {
  handler?: undefined;
  name: 'plain';
}

export function callHandler(value: WithHandler | WithoutHandler): 'handled' | 'plain' {
  if (!value.handler) {
    return value.name;
  }
  value.handler(1);
  return value.name;
}

interface Empty {
  kind?: {};
  tag: 'empty';
}
interface Filled {
  kind: { id: number };
  tag: 'filled';
}

export function emptyStaysWide(value: Empty | Filled): 'empty' | 'filled' {
  if (value.kind) {
    return value.tag;
  }
  return value.tag;
}

export function reportsWhenBothPossible(value: Empty | Filled): Filled | null {
  if (value.kind) {
    const filled: Filled = value;
    return filled;
  }
  return null;
}
