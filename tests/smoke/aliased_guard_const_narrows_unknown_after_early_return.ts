declare function isError(value: unknown): value is Error;

function classify(error: unknown): boolean {
  const isValid = error && isError(error) && typeof error === "object";
  if (!isValid) {
    return false;
  }

  const { message } = error;
  return message !== undefined;
}
