declare const xx: any;
declare function f(): number;

export const asCast = (xx as any, 100);
export const indirect = (0, xx.fn)();
export const indirectElement = (0, xx["fn"])();
export const indirectTag = (0, xx.fn)``;
export const indirectEval = (0, eval)("1");
export const called = (f(), 3);

export const discarded = (xx, 1);
export const logical = (xx || 1, 2);
export const notCalled = (0, xx.fn);
export const doubled = ((0, xx.fn))();
export const nonZero = (1, xx.fn)();
