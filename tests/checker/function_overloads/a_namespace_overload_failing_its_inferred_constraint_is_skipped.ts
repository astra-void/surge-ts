// @filename: lib.ts
export namespace Hooks {
  export type ReducerWithoutAction<S> = (prevState: S) => S;
  export type Reducer<S, A> = (prevState: S, action: A) => S;
  export type ReducerState<R extends Reducer<any, any>> = R extends Reducer<infer S, any> ? S : never;
  export type ReducerStateWithoutAction<R extends ReducerWithoutAction<any>> = R extends ReducerWithoutAction<infer S> ? S : never;
  export function useReducer<R extends ReducerWithoutAction<any>, I>(reducer: R, initializerArg: I, initializer: (arg: I) => ReducerStateWithoutAction<R>): [ReducerStateWithoutAction<R>, () => void];
  export function useReducer<R extends ReducerWithoutAction<any>>(reducer: R, initializerArg: ReducerStateWithoutAction<R>, initializer?: undefined): [ReducerStateWithoutAction<R>, () => void];
  export function useReducer<R extends Reducer<any, any>>(reducer: R, initialState: ReducerState<R>, initializer?: undefined): [ReducerState<R>, (action: number) => void];
  export function useReducer(...args: any[]): any { return args; }
}
// @filename: main.ts
import { Hooks } from "./lib";
type State = { success: boolean };
const initialState: State = { success: false };
const fetchReducer = (state: State, action: number): State => state;
const [state, dispatch] = Hooks.useReducer(fetchReducer, initialState);
export const unchanged: string = dispatch;
