export {};
const key = "k";
const property = { a?: 1 };
const method = { m?() { return 1; } };
const shorthand = { key? };
const mixed = { [key]?: 2, "quoted"?: 3, 4?: 5 };
const read: string = property.a;
