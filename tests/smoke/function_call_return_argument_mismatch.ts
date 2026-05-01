function getAge(): number {
  return 1;
}

function takesString(value: string) {}
takesString(getAge());
