type MaybeHandler = (value?: string) => void;

function callMaybe(fn: MaybeHandler): void {
  fn();
}
