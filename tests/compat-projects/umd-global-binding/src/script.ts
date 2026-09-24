const port: string = Lib.start({ port: 1 });
const options: Lib.Options = { port: "x" };
const version: number = Lib.version;
const widget: WidgetLib = new WidgetLib();
const size: string = widget.size();
const config: WidgetLib.Config = { width: 1 };
const level: string = Tools.level;
