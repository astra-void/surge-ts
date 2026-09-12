import lib from "lib";

declare const options: lib.Options;
declare const server: lib.Server;
declare const listener: lib.Listener;

export const port: number = options.port;
export const serverPort: number = server.options.port;
listener(server);

export const wrongPort: string = options.port;
export const wrongServer: string = server.options.port;
