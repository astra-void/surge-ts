import * as Imported from "./exported-namespace";
import type {
  DerivedConfig,
  DerivedTagged,
  DerivedWrapper,
} from "./ambient-namespace";
import { DerivedEmitter } from "./ambient-namespace";
import type { LocalDerived, LocalNestedDerived } from "./local-namespace";

export interface ImportedPoint extends Imported.Point {
  label: string;
}

export interface ImportedBoxed extends Imported.Boxed<string> {
  count: number;
}

export interface GlobalRequest extends Express.Request {
  method: string;
}

declare const point: ImportedPoint;
declare const boxed: ImportedBoxed;
declare const request: GlobalRequest;
declare const config: DerivedConfig;
declare const wrapper: DerivedWrapper;
declare const tagged: DerivedTagged;
declare const local: LocalDerived;
declare const nested: LocalNestedDerived;
declare const emitter: DerivedEmitter;

export const px: number = point.x;
export const py: number = point.y;
export const plabel: string = point.label;

export const bvalue: string = boxed.value;
export const bcount: number = boxed.count;

export const rurl: string = request.url;
export const ruser: string = request.user.id;
export const rmethod: string = request.method;

export const clevel: number = config.level;
export const clabel: string = config.label;

export const wunwrap: string = wrapper.unwrap();
export const wcached: boolean = wrapper.cached;

export const ttag: string = tagged.tag;
export const torder: number = tagged.order;

export const lid: string = local.id;
export const lextra: boolean = local.extra;

export const ndepth: number = nested.depth;
export const nnote: string = nested.note;

export const ename: string = emitter.name;
export function emitOnce(): void {
  emitter.emit("ready");
}
