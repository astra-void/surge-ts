let optionalMiddle: [number, string?, boolean?];
optionalMiddle = [42, , true];
const undefinedSlot: [number, string | undefined, boolean] = [1, , false];
const holes = [1, , 3];

export const element: number = holes[1];
export const pair: [number, number] = [, 2];
export const numbers: number[] = [1, , 3];
export const wrongAfterHole: [number, string?, boolean?] = [1, , "not a boolean"];
export { undefinedSlot };
