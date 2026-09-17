const tagKey = "tag";

export const record = {
  get [tagKey]() {
    const wrong: string = 1;
    return "record";
  },
  get [Symbol.toStringTag]() {
    const bad: number = "x";
    return "Record";
  },
};

export class Model {
  get [tagKey]() {
    const wrong: string = 2;
    return "model";
  }
  get [Symbol.toStringTag]() {
    const bad: number = "y";
    return "Model";
  }
}
