// `this.p` is a reference like any other; `this` is bound as a symbol.
declare function want(s: string): void;
export class K {
p?: string;
m(): void {
if (this.p) {
want(this.p);
}
}
}
