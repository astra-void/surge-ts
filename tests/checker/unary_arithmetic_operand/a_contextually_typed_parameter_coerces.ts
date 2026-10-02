type Sel = (data: string) => [string, number];
export const sel: Sel = (data) => [data, +data];
