// @surge-args: --diagnosticProfile native
// @surge-expect: none
// @filename: cache.d.ts
// An overload group folds into one callable shape: a call selecting a later
// overload is accepted rather than checked against the first signature only.
export declare function cacheLife(profile: "default"): void;
export declare function cacheLife(profile: "minutes"): void;
export declare function cacheLife(profile: "hours"): void;
// @filename: index.ts
import { cacheLife } from "./cache";
cacheLife("minutes");
cacheLife("hours");
