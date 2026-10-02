interface Query { setState(s: object): void }
declare function find(): Query | undefined;
export function negated(mode: string) {
const query = find();
if (!query && mode === 'reset') { query.setState({}); }
}
