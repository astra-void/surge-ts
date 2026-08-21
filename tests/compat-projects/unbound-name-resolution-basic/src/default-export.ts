// `export default <expr> satisfies T` is still an expression and must be
// checked. It used to parse as unsupported, so nothing inside it was ever seen.
interface Config {
  env: { ci: boolean };
}
export default {
  env: { ci: !!notDeclaredInThisFile },
} satisfies Config;
