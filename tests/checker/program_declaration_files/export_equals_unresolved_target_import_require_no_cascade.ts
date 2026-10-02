// @module: preserve
// @surge-args: --diagnosticProfile native
// @surge-expect: TS2304
// @filename: pkg.d.ts
// The exported identifier is undefined in the package surface; the consumer
// binds an unknown value and must not cascade name/property errors.
export = missingValue;
// @filename: consumer.ts
import api = require("./pkg");
const result = api.whatever();
