export abstract class Cache {
  abstract strategy(): 'explicit' | 'all';
  abstract get(key: string): Promise<unknown[] | undefined>;

  describe(): string {
    return this.strategy();
  }
}

export class Overloaded {
  pick(value: string): string;
  pick(value: number): number;
  pick(value: string | number): string | number {
    return value;
  }
}

export declare class Ambient {
  compute(): number;
}

export class StillReportsARealMissingReturn {
  compute(): number {}
}
