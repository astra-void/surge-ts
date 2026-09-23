enum Direction { Up, Down }
export const name: string = Direction[0];
export const roundTrip: string = Direction[Direction.Up];
enum Mixed { A = 1, B = "b" }
export const mixed: string = Mixed[1];
enum Label { A = "a" }
export const label = Label["a" as string];
