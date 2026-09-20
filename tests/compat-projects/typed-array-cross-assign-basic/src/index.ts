declare function takesInt8(view: Int8Array): void;

export function crossAssign(int8: Int8Array, uint8: Uint8Array, float32: Float32Array): void {
  int8 = uint8;
  int8 = float32;
  takesInt8(uint8);
  takesInt8(int8);
  let view = new Int8Array(1);
  view = uint8;
}
