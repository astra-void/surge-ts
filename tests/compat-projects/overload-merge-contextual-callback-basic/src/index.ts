export function fromIterable<TYield>(
  iterable: AsyncIterable<TYield, void>,
): ReadableStream<TYield> {
  const iterator = iterable[Symbol.asyncIterator]();
  return new ReadableStream({
    async cancel() {
      await iterator.return?.();
    },
    async pull(controller) {
      const result = await iterator.next();
      if (result.done) {
        controller.close();
        return;
      }
      controller.enqueue(result.value);
    },
  });
}

export function listen(socket: WebSocket): void {
  socket.addEventListener('close', (event) => {
    void event.code;
  });
  socket.addEventListener('message', ({ data }) => {
    void data;
  });
}

interface Event0 {
  z: number;
}
interface Listener {
  (event: Event0): void;
}
interface Emitter {
  on(type: string, listener: (event: Event0) => unknown): void;
  on(type: number, listener: Listener): void;
}

export function subscribe(emitter: Emitter): void {
  emitter.on('x', (event) => {
    void event.z;
  });
}

type Ambiguous = ((a: string) => void) | ((a: number, b: string) => void);
declare function callAmbiguous(handler: Ambiguous): void;

export function stillImplicitAny(): void {
  callAmbiguous((value) => {
    void value;
  });
}
