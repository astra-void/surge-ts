type Handler = (this: Unresolved, value: string) => void;

declare const handler: Handler;

handler("ok");
