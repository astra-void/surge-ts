interface User {
  id: number;
  name: string;
  active: boolean;
}

type Flags = Record<"debug" | "trace", boolean>;

const recordOk: Flags = { debug: true, trace: false };
const recordMissing: Flags = { debug: true };
const recordExtra: Flags = { debug: true, trace: false, other: true };
const recordMismatch: Flags = { debug: "yes", trace: false };

type UserPatch = Partial<User>;

const partialOk1: UserPatch = {};
const partialOk2: UserPatch = { id: 1 };
const partialMismatch: UserPatch = { name: 123 };
const partialExtra: UserPatch = { id: 1, unknown: true };

type UserName = Pick<User, "id" | "name">;

const pickOk: UserName = { id: 1, name: "Ada" };
const pickMissing: UserName = { id: 1 };
const pickExtra: UserName = { id: 1, name: "Ada", active: true };
const pickMismatch: UserName = { id: "1", name: "Ada" };

type InvalidPick = Pick<User, "id" | "missing">;
let invalidPickUsage: InvalidPick;

type PublicUser = Omit<User, "active">;

type OmitMissing = Omit<User, "active" | "missing">;

const omitOk: PublicUser = { id: 1, name: "Ada" };
const omitMissing: PublicUser = { id: 1 };
const omitExtra: PublicUser = { id: 1, name: "Ada", active: true };
const omitMismatch: PublicUser = { id: "1", name: "Ada" };
const omitMissingKeyOk: OmitMissing = { id: 1, name: "Ada" };

type UserAll = Pick<User, keyof User>;

const pickAllOk: UserAll = { id: 1, name: "Ada", active: true };
const pickAllMissing: UserAll = { id: 1, name: "Ada" };

type UserMap = Record<keyof User, string>;

const recordKeyofOk: UserMap = { id: "1", name: "Ada", active: "yes" };
const recordKeyofMismatch: UserMap = { id: 1, name: "Ada", active: "yes" };
