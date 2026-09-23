declare const q3: { b: any } | undefined;
declare const o3: { x: number };
({ ...q3?.b } = o3);
export {};
