type Kind = "a" | "b" | "c";
export const f = (k: Kind): number => {
  switch (k) {
    case "a":
    case "b":
      return 1;
    case "c":
      return 2;
  }
};
export function h(k: Kind): number {
  switch (k) {
    case "a":
    case "b":
      return 1;
    case "c":
      return 2;
  }
}
export function missing(k: Kind): number {
  switch (k) {
    case "a":
    case "b":
      return 1;
  }
}
