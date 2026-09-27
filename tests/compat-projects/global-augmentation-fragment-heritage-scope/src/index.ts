export const emitted: boolean = emitter.emitNodeEvent("ready");
export const port: number = address.port;
export const href: string = address.href;
emitter.on({ label: "global-meaning" });

export const wrongEmit: string = emitter.emitNodeEvent("ready");
export const wrongPort: string = address.port;
