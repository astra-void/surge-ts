export type VerifiedRegistrationResponse = {
  verified: boolean;
  registrationInfo?: {
    aaguid: string;
    credential: {
      id: string;
      publicKey: Uint8Array;
    };
    userVerified: boolean;
  };
};
