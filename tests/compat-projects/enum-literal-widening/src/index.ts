enum Direction { Up, Down }
enum Status { Active = "active", Inactive = "inactive" }
namespace Palette {
  export enum Color { Red, Blue }
}

export function toggle(flag: boolean) {
  let direction = Direction.Up;
  direction = Direction.Down;
  let status = Status.Active;
  if (status === Status.Inactive) {
    return;
  }
  status = Status.Inactive;
  let color = Palette.Color.Red;
  color = Palette.Color.Blue;
  const settings = { direction: Direction.Up };
  settings.direction = Direction.Down;
  const history = [Status.Active];
  history.push(Status.Inactive);
  const pinned = Direction.Up;
  let copy = pinned;
  copy = Direction.Down;
  const wrong: Status = Direction.Up;
  return [direction, status, color, settings, history, copy, wrong, flag];
}

export class Machine {
  state = Status.Active;
  stop() {
    this.state = Status.Inactive;
  }
}
