import React from "react";
import * as Zustand from "zustand";
import { createStore } from "zustand/vanilla";
import type { StoreApi } from "zustand";
import "reflect-metadata";

export { createStore } from "zustand/vanilla";
export type { StoreApi } from "zustand";
export * from "zustand/middleware";

let store = createStore();
let api: StoreApi = { getState: 123 } as any;
let element = React;
let namespaceValue = Zustand;
