interface Subscription<T> {
  unsubscribe(): void;
  readonly last: T;
}

interface Envelope<T> {
  readonly payload: T;
  readonly at: number;
}

declare function onMessage(
  handler: (envelope: Envelope<string>) => void,
): Subscription<Envelope<string>>;

declare function onCount(
  handler: (envelope: Envelope<number>) => void,
): Subscription<Envelope<number>>;

const messages = onMessage((envelope) => {
  const text: string = envelope.payload;
  const at: number = envelope.at;
  void text;
  void at;
});

const counts = onCount((envelope) => {
  const total: number = envelope.payload + 1;
  void total;
});

const lastMessage: string = messages.last.payload;
const lastCount: number = counts.last.payload;
messages.unsubscribe();

const bad: number = messages.last.payload;

export { lastMessage, lastCount, bad };
