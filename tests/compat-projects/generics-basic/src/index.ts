import { Box, StoreApi } from "./box";

let box: Box<string> = { value: "ok" };
let store: StoreApi<string> = {
  getState: () => "ok",
};
