declare const maybe: string | undefined;
declare const text: string;

const anchored: number = maybe?.anchor("top");
const direct: number = text.anchor("top");
export {};
