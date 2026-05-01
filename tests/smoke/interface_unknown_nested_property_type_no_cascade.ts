interface Profile {
  displayName: Missing;
}

interface User {
  profile: Profile;
}

let user: User = { profile: { displayName: "Ada" } };
