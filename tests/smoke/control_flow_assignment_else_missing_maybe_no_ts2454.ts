function f(flag: boolean): string {
  let value: string;
  if (flag) {
    value = "a";
  }

  // Conservative on purpose: single-branch assignment stays MaybeAssigned for now.
  return value;
}
