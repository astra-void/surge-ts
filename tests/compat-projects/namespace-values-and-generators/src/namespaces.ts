namespace Shapes {
    export var count = 1;
    export const label = "shape";
    export function area(side: number): number { return side * side; }
    export class Square { side = 1; }
    function hidden(): void {}
    var secret = 2;
}

export const count: string = Shapes.count;
export const label: number = Shapes.label;
export const area: string = Shapes.area(2);
Shapes.area("2");
export const side: string = new Shapes.Square().side;
Shapes.hidden();
Shapes.secret;
Shapes.Square();
