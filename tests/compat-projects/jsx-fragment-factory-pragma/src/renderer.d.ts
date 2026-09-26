declare global {
    namespace JSX {
        interface IntrinsicElements { [name: string]: any }
    }
}
export function dom(): void;
export function frag(): void;
