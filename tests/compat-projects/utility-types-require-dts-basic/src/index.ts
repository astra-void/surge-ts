interface Account {
  id: string;
  label: string;
  active: boolean;
}

type PartialAccount = Partial<Account>;
type AccountLabel = Pick<Account, "label">;
type AccountById = Record<string, Account>;
type AccountWithoutId = Omit<Account, "id">;

declare function makeAccount(id: string): Account;
type MakeReturn = ReturnType<typeof makeAccount>;
type MakeParams = Parameters<typeof makeAccount>;

const partial: PartialAccount = {};
const label: AccountLabel = { label: "ok" };
const byId: AccountById = {};
const without: AccountWithoutId = { label: "ok", active: true };
const made: MakeReturn = { id: "1", label: "a", active: true };
const params: MakeParams = ["1"];

export { partial, label, byId, without, made, params };
