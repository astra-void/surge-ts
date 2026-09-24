import exporter = require("./exporter");
import * as namespace from "./exporter";

function declaredFirst() {}
declaredFirst();

export class Uses {
    static fromEquals(value = exporter.make()) {
        return value;
    }
    static fromNamespace(value = namespace.make()) {
        return value;
    }
    static missing(value = notDeclared.make()) {
        return value;
    }
}
