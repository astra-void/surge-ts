declare const words: string[];
declare const pair: [number, number];
declare function callback(): void;

export const copied: string[] = words.copyWithin(0, 1);
export const anchored: string = "text".anchor("name");
export const anchoredWrong: number = "text".anchor("name");
export const fromTuple = pair.copyWithin(0, 1);
export const exponent: string = (12).toExponential(2);
export const caller = callback.caller;
export const argumentsObject = callback.arguments;

export const notInThisLib = words.at(0);
export const notInThisLibString = "text".padStart(8);
export const neverDeclared = words.definitelyNotAMember;
