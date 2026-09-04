declare const numeric: number;
declare const handle: Fake.Handle | undefined;

export function stopBoth() {
  stopTimer(numeric);
  stopTimer(handle);
}
