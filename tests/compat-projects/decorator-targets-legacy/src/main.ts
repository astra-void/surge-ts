declare function dec(...args: any[]): any;
declare function key(): string;

export class Declared {
    @dec [key()]: any;
    @dec field = 1;
    @dec #hidden = 2;
    @dec method() { }
    @dec get value() { return this.#hidden; }
    constructor(@dec input: number) {
        this.field = input;
    }
    save(@dec first: string, @dec ...rest: string[]) {
        return [first, ...rest];
    }
}

export const Expressed = class {
    @dec [key()]: any;
    @dec field = 1;
    @dec method() { }
    constructor(@dec input: number) {
        this.field = input;
    }
};

export abstract class Base {
    @dec abstract value: number;
    @dec declare other: string;
    @dec [key()]: any;
    @dec abstract run(): void;
    @dec abstract get size(): number;
}
