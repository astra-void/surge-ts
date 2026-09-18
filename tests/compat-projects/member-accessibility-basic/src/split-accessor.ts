export class Connection {
  private current: string | null = null;

  public get socket(): string | null {
    return this.current;
  }

  private set socket(value: string | null) {
    this.current = value;
  }
}

export function inspect(connection: Connection): string | null {
  if (!connection.socket) {
    return null;
  }
  connection.socket = 'reset';
  return connection.socket;
}
