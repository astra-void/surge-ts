// @strict: true
type ResolveResult = string | null | undefined | void | false | { id: string };

interface ResolverContext {
  resolve(id: string, importer: string): Promise<ResolveResult>;
}

type ResolverHook = (
  this: ResolverContext,
  id: string,
  importer: string | undefined,
  options: { isEntry: boolean },
) => ResolveResult;

type MakeAsync<F> = F extends (
  this: infer This,
  ...parameters: infer Arguments
) => infer Return
  ? (this: This, ...parameters: Arguments) => Return | Promise<Return>
  : never;

type ObjectHook<T> = T | { handler: T };
type MapToFunction<T> = T extends Function ? T : never;
type ResolverFunction = MapToFunction<ObjectHook<MakeAsync<ResolverHook>>>;

function makeResolver(): ResolverFunction {
  return async function (id, importer, options) {
    if (importer === undefined) return null;
    const resolved = await this.resolve(id, importer);
    if (resolved) return resolved;
    return options.isEntry ? id : null;
  };
}

function makeSimpleResolver(): ResolverFunction {
  return async function (id) {
    return id;
  };
}

function makeMappedResolver(): MakeAsync<ResolverHook> {
  return async function (id) {
    return id;
  };
}

function makeDirectResolver(): (
  this: ResolverContext,
  id: string,
  importer: string | undefined,
  options: { isEntry: boolean },
) => ResolveResult | Promise<ResolveResult> {
  return async function (id) {
    return id;
  };
}

function makeDirectComplex(): (
  this: ResolverContext,
  id: string,
  importer: string | undefined,
  options: { isEntry: boolean },
) => ResolveResult | Promise<ResolveResult> {
  return async function (id, importer, options) {
    if (importer === undefined) return null;
    const resolved = await this.resolve(id, importer);
    if (resolved) return resolved;
    return options.isEntry ? id : null;
  };
}

function makeMappedComplex(): MakeAsync<ResolverHook> {
  return async function (id, importer, options) {
    if (importer === undefined) return null;
    const resolved = await this.resolve(id, importer);
    if (resolved) return resolved;
    return options.isEntry ? id : null;
  };
}
