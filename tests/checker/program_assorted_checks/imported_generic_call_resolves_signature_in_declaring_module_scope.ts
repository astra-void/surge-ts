// @filename: lib.ts
// Calling an imported generic with explicit type arguments re-resolves its
// declared parameter/return annotations; their names live in the declaring
// module's scope, not the caller's (react-hook-form's
// `useForm(props?: UseFormProps<…>): UseFormReturn<…>`), so the instantiation
// must run under the declaring file or the names report false TS2304s.
export interface Options<T> { seed: T }
export interface Box<T> { value: T }
export function make<T>(options?: Options<T>): Box<T> {
return { value: (options as Options<T>).seed };
}
// @filename: main.ts
import { make } from "./lib";
const box = make<{ email: string }>();
const s: string = box.value.email;
