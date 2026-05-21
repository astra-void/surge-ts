interface Passkey {
  id: string;
}

interface User {
  passkeys?: Passkey[];
}

export function hasNoPasskeys(user: User | null): boolean {
  if (!user || user.passkeys?.length === 0) {
    return true;
  }

  return false;
}
