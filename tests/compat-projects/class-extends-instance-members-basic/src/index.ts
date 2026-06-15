class Base {
  id: string;

  constructor(id: string) {
    this.id = id;
  }

  getId(): string {
    return this.id;
  }
}

class Derived extends Base {
  label: string;

  constructor(id: string, label: string) {
    super(id);
    this.label = label;
  }
}

const d = new Derived("a", "b");

const okId: string = d.id;
const okLabel: string = d.label;
const okMethod: string = d.getId();

const badId: number = d.id;
const badMethod: number = d.getId();
