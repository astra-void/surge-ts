type Mode = "light" | "dark" | "system";

// `default` sees the discriminant with every case value removed.
function pick(mode: Mode): void {
  switch (mode) {
    case "light":
      break;
    case "dark":
      break;
    default: {
      const remaining: "system" = mode;
      const wrong: "light" = mode;
    }
  }
}

// After a `switch` without `default` whose cases all leave, only the
// unmatched values continue.
function after(mode: Mode): string {
  switch (mode) {
    case "light":
      return "l";
    case "dark":
      throw new Error("dark");
  }
  const remaining: "system" = mode;
  return remaining;
}

// A case that breaks still reaches the code after the switch.
function breaking(mode: Mode): void {
  switch (mode) {
    case "light":
      break;
    case "dark":
      return;
  }
  const tooNarrow: "system" = mode;
}

// A `default` grouped with a case keeps that case's value.
function grouped(mode: Mode): void {
  switch (mode) {
    case "light":
      return;
    case "dark":
    default: {
      const rest: "dark" | "system" = mode;
    }
  }
}
