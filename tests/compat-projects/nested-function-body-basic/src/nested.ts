export function outer(run: (callback: () => void) => void) {
  function missing() {
    return notDeclared;
  }
  function readsLaterBinding() {
    return later.length;
  }
  let later = 'set after the function is declared';
  let state: 'idle' | 'pending' | 'done' = 'idle';
  function pull(next: Promise<{ done?: boolean }>) {
    if (state !== 'idle') {
      return;
    }
    state = 'pending';
    next.then((result) => {
      if (result.done) {
        state = 'done';
        return;
      }
      state = 'idle';
    });
  }
  run(() => missing());
  return [readsLaterBinding, pull, later];
}

export class Counter {
  count = 0;
  method() {
    function detached() {
      return count;
    }
    return detached();
  }
}
