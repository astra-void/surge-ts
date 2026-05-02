export interface User {
  name: string;
}

export function getName(): string {
  return "Ada";
}

export const version: string = "1";

export default function getDefaultName(): string {
  return "Ada";
}
