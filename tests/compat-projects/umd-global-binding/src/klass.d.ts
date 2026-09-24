declare class Widget {
  size(): number;
}
declare namespace Widget {
  interface Config { width: number }
}
export = Widget;
export as namespace WidgetLib;
