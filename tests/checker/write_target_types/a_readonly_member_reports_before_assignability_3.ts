class C {
  #v = 0;
  get value(): number { return this.#v; }
  set value(next: number | string) { this.#v = Number(next); }
  get only(): string { return "x"; }
  readonly fixed: number = 1;
}
const c = new C();
c.only = "y";
