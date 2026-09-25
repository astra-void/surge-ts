type Result =
  | { success: true; data: string }
  | { success: false; error: { issues: { message: string }[] } };

type Shape = { kind: "circle"; radius: number } | { kind: "square"; size: number };

export function firstFailure(results: Result[]) {
  if (!results[0].success) {
    return results[0].error.issues[0].message;
  }
  return results[0].data;
}

export function nested(holder: { result: Result }) {
  if (!holder.result.success) {
    return holder.result.error;
  }
  return holder.result.data;
}

export function byIndex(shapes: Shape[], index: number) {
  if (shapes[index].kind === "circle") {
    return shapes[index].radius + shapes[index].size;
  }
  return shapes[index].size;
}
