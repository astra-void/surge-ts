export function ifElseReturn(flag: boolean): string {
  if (flag) {
    return "a";
  } else {
    return "b";
  }
}

export function earlyThrow(flag: boolean): string {
  if (!flag) throw new Error("bad");
  return "ok";
}

export function switchReturn(value: "a" | "b"): string {
  switch (value) {
    case "a":
      return "a";
    case "b":
      return "b";
  }
}
