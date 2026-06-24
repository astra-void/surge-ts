type Strict = string & { foo: number };

declare const plain: string;
const bad: Strict = plain;
