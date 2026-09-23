function f(a = 1) { "use strict"; }
function g({ a }: { a: number }) { "use strict"; }
function h(...a: number[]) { 'use strict'; }
function ok(a: number) { "use strict"; }
function ok2(a = 1) { "x"; "use strict"; }
const arrow = (a = 1) => { "use strict"; };
const arrow2 = (a = 1) => "use strict";
class C { m(a = 1) { "use strict"; } }
function two(a = 1, [b]: number[], c: number) { "use strict"; }
export {};
