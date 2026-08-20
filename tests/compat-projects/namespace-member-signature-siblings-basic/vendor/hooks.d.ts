export = Hooks;

declare namespace Hooks {
  type Dispatch<A> = (value: A) => void;
  type SetStateAction<S> = S | ((previous: S) => S);

  function useBox<S>(initial: S): Dispatch<SetStateAction<S>>;
}
