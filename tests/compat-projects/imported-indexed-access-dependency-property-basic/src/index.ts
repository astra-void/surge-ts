import { VerifiedRegistrationResponse } from "./dep";

declare const verification: VerifiedRegistrationResponse;

const registrationInfo: VerifiedRegistrationResponse["registrationInfo"] =
  verification.registrationInfo;

const credentialId: string | undefined = registrationInfo?.credential.id;
const aaguid: string | undefined = registrationInfo?.aaguid;
const userVerified: boolean | undefined = registrationInfo?.userVerified;
