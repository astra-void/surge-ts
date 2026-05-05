const config = { mode: "dev", retries: 3, enabled: true } as const;

type Config = typeof config;
type Mode = Config["mode"];
type Retries = Config["retries"];
type ConfigKey = keyof Config;
type ConfigValue = Config[ConfigKey];
type ConfigValueDirect = Config[keyof Config];

const modeOk: Mode = "dev";
const modeBad: Mode = "prod"; // TS2322

const retriesOk: Retries = 3;
const retriesBad: Retries = 4; // TS2322

const valueOk1: ConfigValue = "dev";
const valueOk2: ConfigValue = 3;
const valueOk3: ConfigValue = true;
const valueBad: ConfigValue = "prod"; // TS2322

interface User {
  name: string;
  age: number;
  nickname?: string;
}

type UserName = User["name"];
type UserAge = User["age"];
type UserNick = User["nickname"];
type UserValue = User[keyof User];

const userNameOk: UserName = "Ada";
const userNameBad: UserName = 123; // TS2322
const userAgeOk: UserAge = 42;
const userAgeBad: UserAge = "42"; // TS2322

const userValueOk1: UserValue = "Ada";
const userValueOk2: UserValue = 42;
const userValueOk3: UserValue = undefined;
const userValueBad: UserValue = false; // TS2322

type Pair = ["dev", 3];
type First = Pair[0];
type Second = Pair[1];
type OutOfBounds = Pair[2];

const firstOk: First = "dev";
const firstBad: First = "prod"; // TS2322
const secondOk: Second = 3;
const secondBad: Second = 4; // TS2322

// Oracle-backed invalid cases. Keep only if exact codes are understood and implemented.
type MissingConfig = Config["missing"];
type MissingUser = User["missing"];
type BadPrimitiveIndex = string["missing"];
type BadUnknownIndex = unknown["foo"];
type UnresolvedObjectIndex = MissingObject["x"];
type UnresolvedKeyIndex = User[MissingKeyName];
type DirectUnresolvedKeyIndex = User[DoesNotExist];
type BadTupleIndex = Pair["dev"];
type NumberIndex = number["foo"];

let _trigger0: OutOfBounds;
let _trigger1: MissingConfig;
let _trigger2: MissingUser;
let _trigger3: BadPrimitiveIndex;
let _trigger4: BadUnknownIndex;
let _trigger5: UnresolvedObjectIndex;
let _trigger6: UnresolvedKeyIndex;
let _trigger6a: DirectUnresolvedKeyIndex;
let _trigger7: BadTupleIndex;
let _trigger8: NumberIndex;
