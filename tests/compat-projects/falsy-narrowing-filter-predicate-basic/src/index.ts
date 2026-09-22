type Handler = (message: string) => { handled: boolean };

declare const shape: { a: 1 } | undefined;
declare const handler: Handler | undefined;
declare const always: { a: 1 };
declare const text: string | undefined;

if (!shape) {
  const n: string = shape;
}
if (!handler) {
  const n: string = handler;
}
if (!always) {
  const n: string = always;
}
if (!text) {
  const n: number = text;
}

declare const contextual: Handler | undefined;
declare const fallback: Handler;
declare function collect(handlers: Handler[]): void;
collect([contextual, fallback].filter((x) => !!x));
collect([contextual, fallback].filter((x) => x !== undefined));

const truthy: number = [shape].filter((x) => !!x);
const defined: number = [handler].filter((x) => x !== undefined);
const texts: number = [text].filter((x) => !!x);
const identity: number = [shape, handler].filter((x) => x);
