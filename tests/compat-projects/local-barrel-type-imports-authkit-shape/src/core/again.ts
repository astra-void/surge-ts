import { Adapter } from "../adapters/index";
import { Provider } from "../providers/./index";
import { HashingAlgorithm } from "../auth/../auth";

export interface Again {
  adapter: Adapter;
  providers: Provider[];
  algorithm?: HashingAlgorithm;
}
