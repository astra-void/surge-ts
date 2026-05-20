export function annotatedAny(): string {
  try {
    throw new Error("bad");
  } catch (error: any) {
    if (error?.code === "P2002") {
      return "duplicate";
    }
    return String(error.message);
  }
}

export function annotatedObject(): string {
  try {
    throw new Error("bad");
  } catch (error: { code?: string; message: string }) {
    if (error?.code === "P2002") {
      return "duplicate";
    }
    return error.message;
  }
}

export function inferredUnknown(): string {
  try {
    throw new Error("bad");
  } catch (error) {
    return String(error);
  }
}
