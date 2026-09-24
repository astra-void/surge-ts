export namespace Shapes {
    export class Circle { radius = 1; static unit = "cm"; }
    export enum Kind { Round, Square }
    export function area(r: number) { return r * r; }
    export namespace Metrics { export interface Size { w: number } export type Unit = string; }
    export namespace Deep { export const depth = 3; export class Node {} }
}

import Circle = Shapes.Circle;
const circle: Circle = new Circle();
const unit: string = Circle.unit;
import Kind = Shapes.Kind;
const kind: Kind = Kind.Square;
import area = Shapes.area;
const measured: number = area(2);
import Metrics = Shapes.Metrics;
const size: Metrics.Size = { w: 1 };
const suffix: Metrics.Unit = "px";
import S = Shapes;
import Deep = S.Deep;
const depth: number = Deep.depth;
const node: Deep.Node = new Deep.Node();

const wrong: string = circle.radius;

export namespace Consumer {
    import C = Shapes.Circle;
    export function make(): C { return new C(); }
    export function shadowedValue(C: number) { return C + 1; }
    export function typeStillAliased(C: string): C { return new Shapes.Circle(); }
    export function hiddenType<C>(value: C): C { return value; }
}

export { kind, measured, size, suffix, depth, node, unit, wrong };
