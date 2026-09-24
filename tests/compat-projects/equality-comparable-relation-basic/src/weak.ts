export {};
declare const weak: { x?: number };
declare const lengthy: { length?: number };
declare const lit: "a";
declare const str: string;
declare const one: 1;
declare const t: true;
enum E { A = "x" }
declare const member: E;
declare const u: undefined;
declare const flag: boolean;
declare const weakValueOf: { valueOf?: () => boolean };

// accepted
str === weak;
weak === str;
lit === lengthy;
u === weak;
flag === weakValueOf;

// rejected
lit === weak;
weak === lit;
one === weak;
t === weak;
member === weak;
flag === weak;
