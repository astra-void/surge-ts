declare const maybe: undefined | { run: (...args: unknown[]) => number };
declare const optionalMember: undefined | { run?: () => number };
declare const present: { run: () => string };
declare const key: "run";
declare const deep: undefined | { inner: { run(): number } };

export const bracketed: number | undefined = maybe?.["run"]();
export const computed: number | undefined = maybe?.[key](1, 2);
export const nested: number | undefined = deep?.inner["run"]();
export const notNullable: string = present?.["run"]();
export const reallyOptional = optionalMember?.["run"]();
export const tooNarrow: number = maybe?.["run"]();
