interface User {
  name: string;
}

function take(user: User) {}

take({ name: "Ada", age: 36 });
