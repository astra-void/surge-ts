export class Account {
  private balance = 1;
  private label = "";

  get total(): number {
    const text: string = 1;
    return this.balance;
  }

  set total(next: number) {
    const flag: boolean = next;
    this.balance = next;
  }

  get missing() {
    return this.nothing;
  }

  get name(): string {
    return this.label;
  }

  set name(next) {
    const count: number = next;
    this.label = next;
  }
}
