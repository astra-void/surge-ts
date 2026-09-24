declare const spread: ((...items: { x: number }[]) => void) | ((...items: { y: number }[]) => void);
spread({ x: 0, y: 0 }, { x: 1, y: 1 });
spread({ x: 0 });
declare const mixed: ((first: { x: number }, ...rest: { y: number }[]) => void) | ((...items: { x: number }[]) => void);
mixed({ x: 0 }, { x: 0, y: 0 });
mixed({ x: 0 }, { y: 0 });
