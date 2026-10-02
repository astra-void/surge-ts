class C {
constructor(private readonly buf: string) {}
method(): string { return this.buf; }
}
const c = new C("a");
const s: string = c.method();
