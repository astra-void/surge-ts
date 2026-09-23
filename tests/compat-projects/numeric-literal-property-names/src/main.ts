export const values = {
    0b11010: "binary",
    1e1000: true,
    0B11111111111111111111111111111111111111111111111101001010100000010111110001111111111: 1,
    Infinity: "duplicate",
};
values["26"];
values["Infinity"];
values["9.671406556917009e+24"];
values["missing"];

declare const typed: { 1e21: string; 0.0000001: number };
export const big: string = typed["1e+21"];
export const tiny: number = typed["1e-7"];
