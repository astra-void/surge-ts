function take(value: string): string {
  return value;
}

const result = take(true ? (false ? "yes" : "maybe") : "no");
