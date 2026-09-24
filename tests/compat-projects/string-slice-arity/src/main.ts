declare const text: string;
declare const maybe: string | undefined;

export const whole = text.slice();
export const tail = text.slice(1);
export const middle = text.slice(1, 3);
export const nonNull = maybe!.slice();
export const substringNeedsStart = text.substring();
export const substrNeedsStart = text.substr();
export const tooMany = text.slice(1, 2, 3);

export const lower = text.toLocaleLowerCase("en-US");
export const upper = text.toLocaleUpperCase(["de-DE", "ja-JP"]);
export const compared = text.localeCompare(text, ["de-DE"], { sensitivity: "base" });
