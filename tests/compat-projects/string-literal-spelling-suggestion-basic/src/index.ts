type Mode = "light" | "dark" | "system";
type Mixed = "alpha" | "beta" | 1;
type Pair = `${boolean}-${"a" | "b"}`;

const declared: Mode = "lihgt";
const mixedMember: Mixed = "alpah";
const expanded: Pair = "true-c";
const property: { mode: Mode } = { mode: "darkk" };
const element: Mode[] = ["light", "systme"];
const tupleElement: [Mode] = ["drk"];

function returned(): Mode {
  return "sytem";
}

function takes(mode: Mode): void {}
// An argument keeps TS2345: the suggestion only replaces the plain message.
takes("lihgt");

// Too far from every member to suggest anything.
const unrelated: Mode = "zzzzzz";
// Two transpositions cost 2, over the 1.9 budget of a 4-letter name.
const transposed: Mode = "drak";
// Members shorter than 3 characters are only suggested for a case difference.
const short: "ab" | "cd" = "ac";
