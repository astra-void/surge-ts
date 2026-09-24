interface ByString {
  [key: string]: (value: string) => number;
}
interface ByNumber {
  [key: string]: (value: any) => number;
  [key: number]: (value: string) => number;
}
declare const suffix: string;
declare const index: number;

export const byString: ByString = { ["prefix" + suffix]: (value) => value.length };
export const byNumber: ByNumber = { [index + 1]: (value) => value.length };
export const numberFallsBack: ByString = { [index * 2]: (value) => value.length };
export const noContext = { ["prefix" + suffix]: (value) => value };
