interface User {
  id: string;
}

type Bad = User[unknown]; // TS2538
