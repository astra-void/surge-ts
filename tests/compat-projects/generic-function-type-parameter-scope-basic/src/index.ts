export async function request<T>(url: string): Promise<T | null> {
  const value = JSON.parse("{}");
  const data = value as T;
  return data;
}

export async function use(): Promise<{ success: boolean } | null> {
  const response = await request<{ success: boolean }>("/api");
  return response;
}
