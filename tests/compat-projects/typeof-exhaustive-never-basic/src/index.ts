// Once every `typeof` case is ruled out, what is left is `never`.
function describe(value: string | number): string {
  if (typeof value === "string") {
    return "text";
  }
  if (typeof value === "number") {
    return "count";
  }
  const unreachable: never = value;
  return unreachable;
}

function chained(value: boolean | string): void {
  if (typeof value === "boolean") {
  } else if (typeof value === "string") {
  } else {
    const rest: never = value;
  }
}

// A lone primitive tested for another tag is `never` in that branch.
function impossible(value: string): void {
  if (typeof value === "number") {
    value.length;
  }
  if (typeof value !== "string") {
    const other: never = value;
  }
}

// ...but a still-possible member is not.
function incomplete(value: string | number | boolean): void {
  if (typeof value === "string") {
    return;
  }
  const remaining: never = value;
}
