interface User {
  profile: Profile;
}

interface Profile {
  displayName: string;
}

let user: User = { profile: { displayName: "Ada" } };
