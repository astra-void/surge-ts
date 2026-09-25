export class Account {
    private balance = 0;
    private written = 0;
    private ledger: number[] = [];
    #token = "";
    #spent = "";
    private static created = 0;
    private static unused = 0;

    constructor(private owner: string, private readonly region: string, public label: string) {
        this.written = owner.length;
        Account.created++;
    }

    private recurse(depth: number): number {
        return depth > 0 ? this.recurse(depth - 1) : 0;
    }

    private read(key: "balance" | "ledger") {
        return this[key];
    }

    describe() {
        return `${this.region}:${this.#token}:${this.read("balance")}`;
    }

    hasToken(value: object) {
        return #spent in value;
    }
}
