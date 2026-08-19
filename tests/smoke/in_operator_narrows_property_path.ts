type Validation =
  | { includes: string; position?: number | undefined }
  | { startsWith: string };

interface Issue {
  validation: Validation;
}

export function describe(issue: Issue): string {
  if ("includes" in issue.validation) {
    if (typeof issue.validation.position === "number") {
      return String(issue.validation.position);
    }
    return issue.validation.includes;
  }
  return issue.validation.startsWith;
}
