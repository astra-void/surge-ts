import { Account, Savings } from './account';

const account = new Account(1);

export const balance = account.balance;
export const owner = account.owner;
export const id = account.id;
export const count = Account.count;
export const label = Savings.label;

account.audit();
account.balance = 5;

export const escaped = account['balance'];

declare const maybe: Account | undefined;
export const optional = maybe?.balance;

export function read(target: Account): string {
  return target.owner;
}
