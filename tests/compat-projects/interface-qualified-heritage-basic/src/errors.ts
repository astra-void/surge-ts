import * as Imported from "./exported-namespace";
import type { DerivedConfig } from "./ambient-namespace";
import type { LocalNestedDerived } from "./local-namespace";

export interface BadPoint extends Imported.Point {
  label: string;
}

declare const bad: BadPoint;
declare const badConfig: DerivedConfig;
declare const badNested: LocalNestedDerived;

export const wrongInherited: string = bad.x;
export const missingInherited = bad.z;

export interface BadRequest extends Express.Request {
  method: string;
}

declare const badRequest: BadRequest;

export const wrongGlobalMerged: number = badRequest.user.id;
export const missingOnGlobal = badRequest.nope;

export const wrongAmbient: string = badConfig.level;
export const missingAmbient = badConfig.absent;

export const wrongNested: string = badNested.depth;
export const missingNested = badNested.absent;
