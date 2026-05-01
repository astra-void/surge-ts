interface User {
  name: Missing;
}

function make(): User {
  return { name: "Ada" };
}
