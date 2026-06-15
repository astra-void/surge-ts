import { Client } from "pkg";

const client = new Client("https://example.com");
const badClient = new Client(123);

const result = client.request("/x");
const badResult: number = result;

const made = Client.create("https://example.com");
const madeBad: number = made;
