type Event =
  | { type: 'started' }
  | { type: 'state'; state: 'connecting' | 'idle'; error: string };

declare const events: Event[];

const states = events.filter((event) => event.type === 'state');
export const firstError: number = states[0]!.error;

const connecting = events.filter((event) => event.type === 'state' && event.state === 'connecting');
export const connectingError = connecting[0]!.error;

const numbers = [1, 'x', 2].filter((value) => typeof value === 'number');
export const onlyNumbers: string = numbers;

const notOne = [1, 'x'].filter((value) => value !== 1);
export const mixed: string = notOne;

export const handler: { [key: string]: any } = () => 1;
export const described: { label?: string; [key: string]: any } = () => 1;
export const strict: { [key: string]: unknown } = () => 1;
