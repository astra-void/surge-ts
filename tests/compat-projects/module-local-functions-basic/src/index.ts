function helper(value: string): string {
  return value;
}

const arrowHelper = (value: string): string => value;

export function exportedCaller(value: string): string {
  return helper(value);
}

export function exportedArrowCaller(value: string): string {
  return arrowHelper(value);
}

function later(value: string): string {
  return value;
}

export function callsLater(value: string): string {
  return later(value);
}
