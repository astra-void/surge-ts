function take(value: string): string {
  return value;
}

const result = take(true ? (false ? "yes" : 1) : "no");
