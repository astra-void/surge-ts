const encoder = new TextEncoder();
const bytes = encoder.encode("hello");
const byteLength = bytes.length;

const t1: AuthenticatorTransport = "usb";
const t2: AuthenticatorTransport = "internal";
const bad: AuthenticatorTransport = "bluetooth";
