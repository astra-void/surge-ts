export namespace Library {
    export const version = 1;
    export namespace Inner { export class Widget { size = 2; } }
}
import Aliased = Library.version;
export { Aliased };
export import Nested = Library.Inner;
