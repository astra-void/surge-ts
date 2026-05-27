interface User {
  id: string;
}

type Bad = User[MissingKeyName]; // TS2304
