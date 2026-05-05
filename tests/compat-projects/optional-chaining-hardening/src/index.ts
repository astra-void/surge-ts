// 1. Nested Optional Property Chains
type Profile = {
  displayName: string;
};

type User = {
  profile?: Profile;
};

const maybeUser: User | undefined = {} as any;

const ok: string | undefined = maybeUser?.profile?.displayName;
const bad: string = maybeUser?.profile?.displayName; // TS2322

// 2. Optional Property Call / Direct Optional Call Chaining
type User2 = {
  getProfile?: () => Profile;
  getName: () => string;
};

const maybeUser2: User2 | undefined = {} as any;

const profileNameOk: string | undefined = maybeUser2?.getProfile?.()?.displayName;
const profileNameBad: string = maybeUser2?.getProfile?.()?.displayName; // TS2322
const nameOk: string | undefined = maybeUser2?.getName();

// 3. Optional Element Access for Arrays and Tuples
const maybeNames: Array<string> | undefined = ["Ada"] as any;
const firstOk: string | undefined = maybeNames?.[0];
const firstBad: string = maybeNames?.[0]; // TS2322

const maybePair: [string, number] | undefined = ["Ada", 1] as any;
const tupleNameOk: string | undefined = maybePair?.[0];
const tupleAgeOk: number | undefined = maybePair?.[1];
const tupleNameBad: number = maybePair?.[0]; // TS2322

// 4. Nullish Coalescing edge cases
const maybeName: string | undefined = "Ada" as any;
const fallbackOk: string = maybeName ?? "Anonymous";
const fallbackBad: number = maybeName ?? "Anonymous"; // TS2322

const onlyUndefined: undefined = undefined;
const fromUndefinedOk: string = onlyUndefined ?? "fallback";

// 5. Missing / Unresolved no-cascade
const unresolvedOk = missingUser?.name; // TS2304

const maybeUser3: { name: string } | undefined = {} as any;
const missing = maybeUser3?.age; // TS2339
