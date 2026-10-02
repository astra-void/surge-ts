// @strict: true
const defaultDelays = [500, 1000];
const direct: readonly number[] = defaultDelays;
const union: readonly number[] | number[] = defaultDelays;
const narrowed: readonly number[] = union;
function identity(readonlyDelays: readonly number[]): readonly number[] {
  return readonlyDelays;
}

function consume(delays: readonly number[]): void {
  void delays;
}

function run(options: { retryDelays?: readonly number[] }): void {
  consume(options.retryDelays ?? defaultDelays);
}
