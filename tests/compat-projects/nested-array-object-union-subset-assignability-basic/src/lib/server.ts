export type AuthenticatorTransportFuture =
  | "ble"
  | "cable"
  | "hybrid"
  | "internal"
  | "nfc"
  | "smart-card"
  | "usb";

export type PublicKeyCredentialDescriptorJSON = {
  id: string;
  type?: string;
  transports?: AuthenticatorTransportFuture[];
};

export declare function generateAuthenticationOptions(options: {
  rpID: string;
  allowCredentials?: PublicKeyCredentialDescriptorJSON[];
}): Promise<{ challenge: string }>;
