// @strict: true
type AsyncConfig<C> = (phase: string) => Promise<C>;

function wrap<C extends object>(config: C): AsyncConfig<C> {
  return async (phase) => {
    void phase;
    return config;
  };
}

type Resolver = (id: string) => string | undefined | Promise<string | undefined>;

function makeResolver(): Resolver {
  return async function (id) {
    return id;
  };
}

function wrapSpread<C extends object>(config: C): AsyncConfig<C> {
  return async () => {
    return { ...config, rewrites: () => [] };
  };
}

function wrapConditional<C extends object>(
  config: C | ((phase: string) => C | Promise<C>),
): AsyncConfig<C> {
  return async (phase) => {
    const resolved = typeof config === "function" ? await config(phase) : config;
    const direct: C = resolved;
    const copied: C = { ...resolved };
    void direct;
    void copied;
    return { ...resolved, rewrites: () => [] };
  };
}

function copyConditional<C extends object>(config: C | (() => C)): C {
  const resolved = typeof config === "function" ? config() : config;
  return { ...resolved };
}

function copyInlineConditional<C extends object>(config: C | (() => C)): C {
  return { ...(typeof config === "function" ? config() : config) };
}
