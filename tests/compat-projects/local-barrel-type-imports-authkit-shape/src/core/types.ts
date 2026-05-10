import { Adapter } from "../adapters";
import { HashingAlgorithm } from "../auth";
import { Provider } from "../providers";

export interface AuthKitParams {
  adapter: Adapter;
  providers: Provider[];
  algorithm?: HashingAlgorithm;
}
