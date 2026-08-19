type Identity<T> = { [k in keyof T]: T[k] };

type IssueA = Identity<{ code: "a"; path?: (string | number)[]; extra: string }>;
type IssueB = Identity<{ code: "b"; path?: (string | number)[]; other: number }>;

type Issue<T extends { code: string } = IssueA | IssueB> = T extends any ? T : never;

export function first(issues: Issue[]): unknown {
  return issues.every((iss) => iss.path?.[0] !== "x");
}
