import sourceDefault from "./source";
import {
  value,
  mutable,
  fn,
  renamedValue,
  starValue,
  starFn,
  mixedRenamedValue,
  reexportedValue,
  DefaultThing,
  sourceNs,
} from "./barrel";
import type { User, UserId, MixedUser, ReexportedUser } from "./barrel";
import directDefault from "./defaultThing";
import { MissingExport } from "./barrel";
import { MissingModule } from "./missing";

const user: User = { id: "1" };
const userId: UserId = "user-1";
const mixedUser: MixedUser = { id: "3" };
const reexportedUser: ReexportedUser = { id: "2" };
const ok: boolean = sourceDefault.ok;
const total: number = value + mutable + renamedValue + mixedRenamedValue + reexportedValue;
const fnResult: string = fn();
const starTotal: number = starValue;
const starResult: string = starFn();
const nsValue: number = sourceNs.starValue;
const nsFn: string = sourceNs.starFn();

void DefaultThing;
void directDefault;
void user;
void userId;
void mixedUser;
void reexportedUser;
void ok;
void total;
void fnResult;
void starTotal;
void starResult;
void nsValue;
void nsFn;
void MissingExport;
