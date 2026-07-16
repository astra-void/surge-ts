const target = new EventTarget();

for (let index = 0; index < 200; index += 1) {
  target.addEventListener("click", (event) => {
    const valid: Event = event;
    void valid;
  });
}
