function make(flag: boolean): { name: string; age?: number } {
  return flag ? { age: 36 } : { name: "Ada" };
}
