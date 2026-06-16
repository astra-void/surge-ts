export function useGlobals(): void {
  const values: Array<number> = [];
  const pending: Promise<number> = Promise.resolve(1);
  const keys = Object.keys(values);
  const now = Date.now();
  const floor = Math.floor(1.5);
  const json = JSON.stringify(keys);
  console.log(pending, now, floor, json);
}
