interface ClientOptions {
  transformer?: string;
}

declare function resolveTransformer(name: string): number;

export function fromTernary(opts?: ClientOptions): number | undefined {
  return opts?.transformer ? resolveTransformer(opts.transformer) : undefined;
}

export function fromIfStatement(opts?: ClientOptions): number | undefined {
  if (opts?.transformer) {
    return resolveTransformer(opts.transformer);
  }
  return undefined;
}

export function keepsTheChain(opts?: ClientOptions): number | undefined {
  return opts?.transformer ? resolveTransformer(opts?.transformer) : undefined;
}

export function elseBranchProvesNothing(opts?: ClientOptions): number {
  if (opts?.transformer) {
    return 0;
  }
  return resolveTransformer(opts?.transformer);
}
