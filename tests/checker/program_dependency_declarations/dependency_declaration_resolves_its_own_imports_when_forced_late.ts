// @filename: node_modules/shapes/index.d.ts
// A dependency `.d.ts` resolves its own imported names when its declarations
// are forced later (a lazy value annotation peels inside an environment
// captured before the per-file scope map existed). Without the program-wide
// scope fallback, `SVGProps` here missed and the annotation degraded.
export interface Shape { size: number }
// @filename: node_modules/dep/index.d.ts
import { Shape } from 'shapes';
declare const widget: Shape;
export { widget };
// @filename: src/index.ts
import { widget } from 'dep';
export const ok: number = widget.size;
export const bad: string = widget.size;
