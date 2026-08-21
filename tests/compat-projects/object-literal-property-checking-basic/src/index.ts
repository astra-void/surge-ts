// An object literal with no contextual type still checks its property values.
declare const anyValue: any;

export const untypedCallback = { run: (opts) => opts };
export const nestedCallback = { outer: { inner: (opts) => opts } };
export const callbackBody = {
  run: () => {
    const mismatch: number = 'text';
    return mismatch;
  },
};
export const unresolvedValue = { key: missingName };
export const memberOnAny = { key: anyValue.method(missingOnAnyCall) };

// A contextually typed literal keeps its parameter types, so nothing is
// implicitly `any` here.
type Handlers = { run: (value: number) => number };
export const contextual: Handlers = { run: (value) => value };

// Method shorthand is checked once, not twice.
export const shorthand = {
  run(value) {
    return value;
  },
};
