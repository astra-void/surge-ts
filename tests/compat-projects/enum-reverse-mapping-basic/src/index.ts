import { Level, Mode } from "./levels";

enum Direction { Up = 1, Down, Left, Right }

const first = Direction[1];
const firstName: string = first;
const firstCount: number = Direction[2];

declare const position: number;
const byNumber: string = Direction[position];
const shouted = Direction[3].toUpperCase();

for (const key in Direction) {
  const name: string = Direction[key];
}

enum Tone { Warm = "warm", Cool = "cool" }
const toneByIndex = Tone[0];

enum Mixed { Zero = 0, Label = "label" }
const mixedName: string = Mixed[0];

enum Empty {}
const emptyName: string = Empty[0];

enum Single { Only }
const singleName: string = Single[7];

declare function compute(): number;
enum Computed { Value = compute() }
const computedName: string = Computed[0];

enum Split { A = 1 }
enum Split { B = 2 }
const splitName: string = Split[2];

enum Words { A = "a" }
enum Words { B = 3 }
const wordsName: string = Words[3];

const levelName: string = Level[0];
const modeByIndex = Mode[0];

const unknownKey = Direction["Sideways"];

type DirectionKey = keyof typeof Direction;
const directionKey: DirectionKey = "Up";

function local() {
  enum Inner { X }
  const innerName: string = Inner[0];
  enum InnerText { X = "x" }
  const innerText = InnerText[0];
}

export {};
