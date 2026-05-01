type User = { name: string; age?: number };

function choose(flag: boolean): User {
  return flag ? { name: "Ada" } : { age: 1 };
}
