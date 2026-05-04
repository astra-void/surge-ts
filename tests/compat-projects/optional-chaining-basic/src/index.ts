type User = {
  name: string;
  nickname?: string;
  getName: () => string;
  age: number;
};

const maybeUser: User | undefined = {} as any;

const nameOk: string | undefined = maybeUser?.name;
const nameBad: string = maybeUser?.name; // TS2322

const nickOk: string | undefined = maybeUser?.nickname;
const fallbackName: string = maybeUser?.name ?? "Anonymous";
const fallbackBad: number = maybeUser?.name ?? "Anonymous"; // TS2322

const callOk: string | undefined = maybeUser?.getName();
const callBad: string = maybeUser?.getName(); // TS2322
maybeUser?.missing; // TS2339

const maybeFn: (() => number) | undefined = (() => 1) as any;
const fnOk: number | undefined = maybeFn?.();
const fnBad: number = maybeFn?.(); // TS2322