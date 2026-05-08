declare function withCallback(
  cb: (err1: Error | null, err2: Error | null) => void
): void;

export function run(): void {
  withCallback((err1, err2) => {
    console.error(err1, err2);
  });
}

export function noLeak(): void {
  withCallback((err1, err2) => {
    console.error(err1, err2);
  });
  console.error(err1);
}
