function createOtp(): string {
  return "123456";
}

export function useObjectShorthand() {
  const user = { id: "u1" };
  const status = "authenticated";
  return { user, status };
}

export default {
  createOtp,
};
