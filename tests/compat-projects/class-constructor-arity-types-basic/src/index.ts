class User {
  constructor(id: string, count?: number) {}
}

new User("a");
new User("a", 1);

new User();
new User(123);
new User("a", "b");
new User("a", 1, 2);
