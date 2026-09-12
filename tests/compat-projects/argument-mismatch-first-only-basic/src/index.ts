declare function pair(a: string, b: number): void;
declare function tupleRest(...args: [a: string, b: number]): void;
declare function arrayRest(...args: string[]): void;
declare const callable: { (a: string, b: number): void };
declare function nested(a: string, b: number): number;

pair(1, 'a');
tupleRest(1, 'a');
arrayRest(1, 2);
callable(1, 'a');
pair(1, nested(2, 'b'));
pair('ok', 1);
