// Several callable members cannot be told apart without signature matching, so
// the callback must not be typed as one of them — using the parameter as the
// *other* member's type stays clean rather than reporting against the guess.
type H1 = (a: string) => void;
type H2 = (a: number) => void;
interface Opts { on?: H1 | H2 }
declare function request(opts: Opts): void;
request({ on: (a) => { const n: number = a; void n; } });
