// `void` / `delete` / `~` have no modelled result, but their operands are still
// expressions: dropping the whole node stopped them being checked at all.
type Plain = { value: string };
declare const g: Plain;
void g.missing;
