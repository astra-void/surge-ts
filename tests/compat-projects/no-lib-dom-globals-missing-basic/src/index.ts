export function useDomGlobals(): void {
  fetch("https://example.com");
  const request = new Request();
  const response = new Response();
  const url = new URL("https://example.com");
  console.log(request, response, url);
}
