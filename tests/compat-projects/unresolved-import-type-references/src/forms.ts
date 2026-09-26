import * as Namespace from "missing-namespace";
import Default from "missing-default";

export const fromNamespace: Namespace.Handler = (event) => event;
export const fromDefault: Default.Handler = (event) => event;
// The default import itself names the error type, not a value used as a type.
export const bare: Default = 1;

export class FromDefault extends Default {
    run() {
        return this.inherited;
    }
}
