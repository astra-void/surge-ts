class Account {
    id = 1;
    protected balance = 2;
    private secret = "";
    #token = "";
    constructor(protected readonly owner: string, public label: string) {}
    read() {
        return this.#token + this.secret;
    }
}

type Keys = keyof Account;
export const id: Keys = "id";
export const label: Keys = "label";
export const read: Keys = "read";
export const balance: Keys = "balance";
export const owner: Keys = "owner";
export const secret: Keys = "secret";

declare const account: Account;
export const partial: Partial<Account> = account;
export const required: Required<Account> = account;
export const readonly: Readonly<Account> = account;
export const omitted: Omit<Account, "id"> = account;

type Mapped<T> = { [K in keyof T]: T[K] };
export const mapped: Mapped<Account> = account;
