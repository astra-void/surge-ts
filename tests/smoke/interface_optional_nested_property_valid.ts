interface Profile {
  displayName: string;
}

interface User {
  profile?: Profile;
}

let user: User = { profile: { displayName: "Ada" } };
