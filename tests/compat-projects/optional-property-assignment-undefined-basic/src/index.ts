interface AdapterUser {
  id: string;
  email?: string;
}

function mapUser(user: { id: string; email: string | undefined }): AdapterUser {
  return {
    id: user.id,
    email: user.email,
  };
}

interface RequiredEmailUser {
  id: string;
  email: string;
}

function badRequired(user: { id: string; email: string | undefined }): RequiredEmailUser {
  return {
    id: user.id,
    email: user.email,
  };
}

void mapUser;
void badRequired;
