const SCRYPT_PARAMS = {
  N: 2 ** 14,
  r: 8,
  p: 1,
  dkLen: 32,
};

declare function scrypt(options: {
  N: number;
  r: number;
  p: number;
  dkLen?: number;
}): void;

scrypt(SCRYPT_PARAMS);
