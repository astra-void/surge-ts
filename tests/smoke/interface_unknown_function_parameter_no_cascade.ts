interface User {
  name: Missing;
}

function take(user: User) {}

take({ name: "Ada" });
