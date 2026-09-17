export {}
const c = 1;
let n = 1;
declare let s: string;
declare let b: boolean;
declare let bi: bigint;
declare let anyv: any;
declare let un: number | undefined;
declare const ro: { readonly k: number };
declare let mut: { k: number };
declare let nums: number[];
declare let strs: string[];
enum E { A, B }
declare let e: E;

n++;
bi++;
anyv++;
e++;
mut.k++;
nums[0]++;

c++;
--c;
s++;
b++;
strs[0]++;
ro.k++;
un++;

const bigIntKeeps: bigint = bi++;
const numberResult: number = n++;
const notString: string = n++;
