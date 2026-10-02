// @noImplicitAny: true
declare function run(f: (...args: [value: number, flag: boolean]) => void): void;
run((value, flag) => {
const s: string = flag;
void s;
void value;
});
