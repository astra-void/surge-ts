function getName(): string {
  return "Ada";
}

let fn: (() => string) | undefined = getName;
