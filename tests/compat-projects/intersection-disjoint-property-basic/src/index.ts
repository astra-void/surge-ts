declare const label: { text: string | undefined } & { text: string };
export const labelText: number = label.text;

declare const stamp: { at: Date | undefined } & { at: Date };
export const stampAt: number = stamp.at;

type Impossible = string & number;
export const impossible: Impossible = 1;


class Connection {
  socket: Date | undefined = undefined;
  isOpen(): this is { socket: Date } {
    return this.socket !== undefined;
  }
}

class Client {
  private readonly connection: Connection = new Connection();

  send(): void {
    if (!this.connection.isOpen()) {
      throw new Error('closed');
    }
    const opened: number = this.connection.socket;
  }
}
