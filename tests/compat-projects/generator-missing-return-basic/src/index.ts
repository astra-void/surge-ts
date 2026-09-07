type Chunk = { index: number };

export async function* asyncProducer(count: number): AsyncIterable<Chunk, void> {
  for (let index = 0; index < count; index++) {
    yield { index };
  }
}

export function* syncProducer(count: number): Iterable<Chunk> {
  yield { index: count };
}

export class Producer {
  async *chunks(): AsyncIterable<Chunk> {
    yield { index: 0 };
  }
}

export function notAGenerator(count: number): Chunk {
  void count;
}
