// Part A: Non-null assertion
let maybeName: string | undefined;
const okName: string = maybeName!;

let maybeNumber: number | undefined;
const wrongName: string = maybeNumber!;

let maybeUser: { name: string } | undefined;
const okProp: string = maybeUser!.name;

missingValue!;

interface Profile {
  displayName: string;
}
interface User {
  profile?: Profile;
}
let maybeUser2: User | undefined;

const display: string = maybeUser2?.profile!.displayName;

// Part B: as const
const mode = "dev" as const;
const exactMode: "dev" = mode;
const wrongMode: "prod" = mode;

const config = { mode: "dev" } as const;
const exactConfigMode: "dev" = config.mode;

const tuple = ["dev", 1] as const;
const first: "dev" = tuple[0];
const second: 1 = tuple[1];

interface Config {
  mode: "dev" | "prod";
}

const validConfig = { mode: "dev" } as const satisfies Config;
const validMode: "dev" = validConfig.mode;

const invalidConfig = { mode: "stage" } as const satisfies Config;
