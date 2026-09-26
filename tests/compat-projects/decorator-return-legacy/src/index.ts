interface OmniDecorator extends ClassDecorator, PropertyDecorator { }
declare function omni(...args: any[]): OmniDecorator;
declare function sealed(target: Function): void;
declare function keep<T extends Function>(target: T): T;
declare function tag(target: object, key: string | symbol): number;
declare function param(target: object, key: string | symbol | undefined, index: number): string;
declare function anyDec(...args: any[]): any;

@omni
class Decorated {
    @omni omniProperty: any;
    @tag taggedProperty: any;
    @anyDec anyProperty: any;
    constructor(@param x: number) { }
}

@sealed
class Sealed { }

@keep
class Kept { }

export { };
