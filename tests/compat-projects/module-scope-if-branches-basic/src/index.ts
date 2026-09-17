declare const value: string | number;
declare const enabled: boolean;
declare function isSymbol(input: unknown): input is symbol;
declare const answer: string | symbol;

// Each branch of a module-scope `if` is checked under its own narrowing.
if (typeof value === "string") {
  const text: string = value;
  const wrongText: number = value;
} else {
  const count: number = value;
  const wrongCount: string = value;
}

if (enabled) {
  const unrelated: number = "enabled";
}

if (isSymbol(answer)) {
  const symbolic: symbol = answer;
} else if (answer.length > 0) {
  const nonEmpty: string = answer;
  const wrongNonEmpty: boolean = answer;
}
