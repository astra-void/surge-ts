export class ClosedError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ClosedError';
    Object.setPrototypeOf(this, ClosedError.prototype);
  }
}

export const prototypeIsTheInstance: ClosedError = ClosedError.prototype;

type Maybe<T> = T | null | undefined;

export function messageOf(error: Maybe<ClosedError>): string {
  if (error instanceof Error && error.name === 'ClosedError') {
    const message: string = error.message;
    return message;
  }
  return '';
}

export function unrelatedConstructorStillNarrows(
  value: Maybe<ClosedError> | string,
): string {
  if (value instanceof Error) {
    return value.message;
  }
  return typeof value === 'string' ? value : '';
}

class Marker {
  tag = 'marker';
}

export const markerPrototype: Marker = Marker.prototype;

export const prototypeIsNotTheStaticSide: number = Marker.prototype;

interface Message {
  id: number;
}

export function sendOneOrMany(
  messageOrMessages: Message | Message[],
): number {
  const messages =
    messageOrMessages instanceof Array ? messageOrMessages : [messageOrMessages];
  return messages.length;
}
