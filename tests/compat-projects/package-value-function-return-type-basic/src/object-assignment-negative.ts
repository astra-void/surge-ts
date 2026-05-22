interface Verification {
  verified: boolean;
}

interface Passkey {
  id: string;
}

const wrong = {
  verification: {},
  passkey: { id: "p1" },
};

const target: { verification: Verification; passkey: Passkey } = wrong;

void target;
