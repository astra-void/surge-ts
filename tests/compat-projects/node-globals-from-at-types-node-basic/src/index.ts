export function useNode(): void {
  const buffer = Buffer.from("payload");
  const size: number = buffer.length;
  const home: string | undefined = process.env.HOME;
  const timer: NodeJS.Timeout = {} as NodeJS.Timeout;
  console.log(size, home, timer.ref());
}
