import { vi, fn } from './spy';

declare function listen(handler: (value: string) => void): void;

// `vi.fn` is a property typed `typeof fn`; the call still binds `T` — here to
// its default, since nothing infers it — so the result is callable and
// assignable to a callback.
const viaMember = vi.fn();
listen(viaMember);
export const calledMember: unknown = viaMember();

// The direct import goes through the symbol path and binds the same way.
const viaImport = fn();
listen(viaImport);
export const calledImport: unknown = viaImport();

// A chained `this`-returning method keeps the instantiated receiver.
const chained = vi.fn().mockReturnValue(1);
listen(chained);

// Not a suppression: the result is a mock, not a number.
declare const count: number;
export const bad: string = count;
