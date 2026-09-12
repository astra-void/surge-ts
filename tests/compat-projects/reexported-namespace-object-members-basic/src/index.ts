import * as middle from "./middle";

export const missingModuleMember = middle.inner.nonExistentMember;
export const kind: number = middle.inner.thing.kind;

declare const thingType: typeof middle.inner.thing;
export const missingThingMember = thingType.nonExistentThingMember;
