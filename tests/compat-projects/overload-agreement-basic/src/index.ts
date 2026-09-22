export function exported(): void;
function exported() {}

declare function ambient(): void;
function ambient(x?: number) {}

class Members {
  public access(): void;
  private access(x: number): void;
  access(x?: number) {}
  maybe?(): void;
  maybe(x: number): void;
  maybe(x?: number) {}
  static twin(): void;
  twin(): void {}
  other(): void;
  static other(): void {}
}
abstract class Shapes {
  abstract area(): number;
  area(x: number): number;
  area(x?: number): number {
    return 0;
  }
}

function misnamed(): void;
function implementation() {}
