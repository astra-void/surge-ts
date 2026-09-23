namespace Cross { export const y = 2; }
namespace crossFn { export const z = 3; }
namespace AmbientCross { export const w = 4; }
namespace TypesOnly { export type U = number; }
class TypesOnly {}
namespace Early { export const x = 1; }
class Early {}
namespace EarlyFn { export const x = 1; }
function EarlyFn() {}
function LateFn() {}
namespace LateFn { export const x = 1; }
