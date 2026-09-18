export class Account {
  private balance = 0;
  protected owner = 'me';
  private static count = 0;
  protected static label = 'account';

  constructor(private readonly id: number) {}

  private audit(): void {}

  deposit(other: Account): number {
    const self = this;
    [1].forEach(function () {
      void self.balance;
    });
    this.audit();
    Account.count++;
    return this.balance + other.balance + this.id + Account.count;
  }
}

export class Savings extends Account {
  rate(): string {
    return this.owner + Savings.label;
  }

  peek(): number {
    return this.balance;
  }
}
