import type { Listener, Message } from "ambient-lib";
import type { Handler, Payload } from "./source";

declare const listener: Listener;
declare const message: Message;
listener(message);

declare const handler: Handler;
declare const payload: Payload;
export const size: number = handler(payload);

export function instanceOf(ctor: typeof Payload): Payload {
  return new ctor();
}

export const url: string = message.url;

export const wrongUrl: number = message.url;

export const wrongSize: string = handler(payload);
