// A callable object — an interface with a call signature — infers through
// that signature the way a function does.
interface Mock { (...args: any[]): any; calls: number }
declare const mock: Mock;
declare function on<T>(handler: (value: T) => void): T;
export const value: string = on(mock);
