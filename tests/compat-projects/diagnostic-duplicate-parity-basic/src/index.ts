const a: string = 1;
const b: string = 2;

let c: MissingType;
let d: MissingType;

interface User {
  id: string;
}

const u1: User = { id: 1 };
const u2: User = { id: 2 };

function takesUser(user: User) {}

takesUser({ id: 1 });
takesUser({ id: 2 });
