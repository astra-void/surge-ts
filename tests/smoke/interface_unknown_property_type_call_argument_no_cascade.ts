interface User {
  name: Missing;
}

function take(user: User): string {
  return "ok";
}

take({ name: "Ada" });
