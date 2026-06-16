export async function callApi(): Promise<unknown> {
  const request = new Request("https://example.com");
  const response: Response = await fetch(request);
  const url = new URL("https://example.com");
  console.log(url.href, response.ok);
  return response.json();
}
