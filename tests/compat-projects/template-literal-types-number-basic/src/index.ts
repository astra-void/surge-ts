type Status = `status:${200 | 404}`;

const ok: Status = "status:200";
// `status:999` is far enough from every member that tsc reports a plain
// TS2322 rather than the TS2820 "Did you mean ..." spelling-suggestion variant,
// which this checker does not emit. The template expansion itself is exercised
// identically either way.
const bad: Status = "status:999";
