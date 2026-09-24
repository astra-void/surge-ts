# namespace-member-declarations-checked

tsc's `checkModuleDeclaration` checks a namespace body with `checkSourceElement`,
exactly as it checks a file: an interface, type alias or class declared inside a
namespace — exported or not, nested, dotted (`namespace P.Q`) or ambient — has
its member types resolved and reported (TS2304). Bare references to the
namespace's own members still resolve.
