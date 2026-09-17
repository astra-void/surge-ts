function withRest(first: number, ...rest: string[]): void {}
function withOptional(first: number, second?: string): void {}
function exact(first: number, second: string): void {}
const arrowWithRest = (first: number, ...rest: number[]) => first;

// A rest parameter makes the minimum the only bound.
withRest();
arrowWithRest();

// Optional parameters make the expected count a range.
withOptional();
withOptional(1, "a", 3);

// Without either, the count is exact.
exact(1);
exact(1, "a", true);
