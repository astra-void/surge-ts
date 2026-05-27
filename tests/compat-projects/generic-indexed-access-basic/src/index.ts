interface User {
  id: string;
  age: number;
  nickname?: string;
}

type Prop<T> = T["value"];
type PickValue<T, K> = T[K];
type Values<T> = T[keyof T];
type OptionalProp<T> = T["nickname"];

type PropValue = Prop<{ value: string }>;
type PropValueMismatch = Prop<{ value: number }>;
type PickId = PickValue<{ id: string; age: number }, "id">;
type PickAge = PickValue<{ id: string; age: number }, "age">;
type UserValues = Values<{ name: string; age: number }>;
type OptionalNickname = OptionalProp<{ nickname?: string }>;

const propOk: PropValue = "ok";
const propBad: PropValue = 123; // TS2322

const propMismatch: PropValueMismatch = "ok"; // TS2322

const pickIdOk: PickId = "user";
const pickIdBad: PickId = 123; // TS2322
const pickAgeOk: PickAge = 42;
const pickAgeBad: PickAge = "42"; // TS2322

const valuesOk1: UserValues = "Ada";
const valuesOk2: UserValues = 42;
const valuesBad: UserValues = true; // TS2322

const optionalOk1: OptionalNickname = "Ada";
const optionalOk2: OptionalNickname = undefined;
const optionalBad: OptionalNickname = 123; // TS2322

type UnknownReceiver = MissingObject["x"]; // TS2304
type UnknownKey = User[MissingKeyName]; // TS2304
type UnknownIndex = User[unknown]; // TS2538
