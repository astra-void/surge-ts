// A quoted key names an object-literal property: dropping it inferred a
// fully-quoted literal (Prisma's generated client config) as `{}` and reported
// every required property as missing.
type Config = { previewFeatures: string[]; clientVersion: string };
const config: Config = {
"previewFeatures": [],
"clientVersion": "7.8.0",
};
export const use = config;
