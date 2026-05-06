import {
  User,
  UserId,
  value,
  fn,
  mixedRenamedValue,
  reexportedValue,
  DefaultThing,
  userNs,
  ReexportedUser,
  MissingExport,
} from "../types/barrel";
import { MissingModule } from "../types/missing";

const user: User = { id: "1" };
const userId: UserId = "user-1";
const reexportedUser: ReexportedUser = { id: "2" };
const total: number = value + mixedRenamedValue + reexportedValue;
const fnResult: number = fn("ok");
const nsValue: string = userNs.value;
const nsFn: number = userNs.fn("ok");

void DefaultThing;
void user;
void userId;
void reexportedUser;
void total;
void fnResult;
void nsValue;
void nsFn;
void MissingExport;
