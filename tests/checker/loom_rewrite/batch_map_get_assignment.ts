// @strict: true
interface Batch {
  ids: Map<string, number[]>;
  timer: number | undefined;
  retryDelays: readonly number[];
}

const batches = new Map<string, Batch>();

function enqueue(size: string, retryDelays: readonly number[]): void {
  let batch = batches.get(size);
  if (!batch) {
    batch = { ids: new Map(), timer: undefined, retryDelays };
    batches.set(size, batch);
  }
  const open = batch;
  open.ids.set(size, []);
  if (open.timer !== undefined) {
    clearTimeout(open.timer);
  }
}
