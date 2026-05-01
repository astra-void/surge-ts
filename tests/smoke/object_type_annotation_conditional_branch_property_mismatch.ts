function make(flag: boolean): { name: string } {
  return flag ? { name: "Ada" } : { name: 123 };
}
