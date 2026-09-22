type Slash = `/${string}`;
type Px = `${number}px`;
type Protocol = `${"http" | "ftp"}://${string}`;

declare function download(spec: Protocol): void;

export function literals(s: string): void {
  const a: Slash = "/bin";
  const b: Slash = "no slash";
  const c: Px = "12px";
  const d: Px = "abpx";
  const e: Slash = s;
  download("http://example.com");
  download("gopher://example.com");
}

export function expressions(s: string, n: number): void {
  const a: `abc${string}` = `abc${s}`;
  const b: `abc${number}` = `abc${n}`;
  const c: `abc${number}` = `abc${s}`;
  const plain = `abc${s}`;
  const d: `abc${string}` = plain;
  let x: `*${number}*`;
  x = `*${n}*` as const;
  x = `*${s}*` as const;
}

export function wrappers(t: `foo${number}`, s: string): void {
  const a: string = t;
  const b: String = t;
  const c: String = s;
  const d: Number = s;
}

type Handler = `on${Capitalize<string>}`;

export function mappings(s: string, upper: Uppercase<string>, lower: Lowercase<string>): void {
  const a: Uppercase<string> = "ABC";
  const b: Uppercase<string> = "aBC";
  const c: Uppercase<string> = s;
  const d: Handler = "onClick";
  const e: Handler = "onclick";
  const f: Uppercase<"abc" | "de"> = "DE";
  const g: Uppercase<string> = upper;
  const h: Uppercase<string> = lower;
  const i: string = upper;
}
