const config = { mode: "dev", retries: 3 } as const;

type Config = typeof config;
type ConfigKey = keyof Config;
type ConfigKeyDirect = keyof typeof config;

const okKey: ConfigKey = "mode";
const okKey2: ConfigKeyDirect = "retries";
const badKey: ConfigKey = "missing"; // TS2322

const configCopy: Config = { mode: "dev", retries: 3 } as const;
const badConfigCopy: Config = { mode: "prod", retries: 3 } as const; // TS2322

interface User {
  name: string;
  age: number;
  nickname?: string;
}

type UserKey = keyof User;
const userKeyOk: UserKey = "nickname";
const userKeyBad: UserKey = "missing"; // TS2322

function getName(): string {
  return "Ada";
}

type GetName = typeof getName;
const fnOk: GetName = getName;
const fnBad: GetName = "not a function"; // TS2322

type MissingTypeof = typeof missingValue; // TS2304
let m: MissingTypeof; // force usage
type MissingKeyof = keyof UnresolvedKeyofTarget; // TS2304
let k: MissingKeyof; // force usage
