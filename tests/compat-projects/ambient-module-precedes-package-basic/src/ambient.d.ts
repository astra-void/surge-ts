declare module 'mypkg' {
  interface ParsedQuery {
    [key: string]: string | undefined;
  }
  function parse(input: string): ParsedQuery;
}
