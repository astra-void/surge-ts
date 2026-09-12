import Stream from "stream-like";
import Emitter, { captureRejections, defaultMaxListeners } from "events-like";
import { Base, Derived } from "named-like";

export const stream = new Stream();
export const marker: number = Stream.streamMarker;
export const rejections: boolean = captureRejections;
export const viaBase: number = Emitter.defaultMaxListeners;
export const base: number = Base.limit;
export const derivedLimit: number = Derived.limit;
export const shared: number = Derived.shared;

export const wrongShared: string = Derived.shared;
export const wrongListeners: string = defaultMaxListeners;
