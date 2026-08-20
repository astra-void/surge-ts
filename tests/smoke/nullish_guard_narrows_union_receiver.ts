interface A {
  code: "a";
  message?: string | undefined;
}
interface B {
  code: "b";
  message?: string | undefined;
}

type Either = (A | B) & { fatal?: boolean | undefined };

export function describe(issue: Either): string {
  if (issue.message !== undefined) {
    return issue.message;
  }
  return "";
}
