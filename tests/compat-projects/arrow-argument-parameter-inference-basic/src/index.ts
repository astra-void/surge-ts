import { vi, fn } from './spy';

type Transformer = (value: Date) => string;
declare function serialize(value: Date, transform?: Transformer): string;

// `T` binds to the arrow's own signature — its annotated parameter and the
// return the body computes through that parameter — so the mock is callable
// and satisfies the callback.
const iso = vi.fn((value: Date) => value.toISOString());
serialize(new Date(), iso);
export const viaMember: string = iso(new Date());

// The direct import goes through the symbol path and binds the same way.
const direct = fn((value: Date) => value.toISOString());
serialize(new Date(), direct);
export const viaImport: string = direct(new Date());

// An `any` parameter is still a source; the return is `any`.
const passthrough = vi.fn((data: any) => data);
serialize(new Date(), passthrough);

// A block body with a straight-line return binds through the same parameter.
const upper = vi.fn((value: Date) => {
  return value.toISOString().toUpperCase();
});
serialize(new Date(), upper);

// Not a suppression: the result is a mock, not a number.
declare const count: number;
export const bad: string = count;
