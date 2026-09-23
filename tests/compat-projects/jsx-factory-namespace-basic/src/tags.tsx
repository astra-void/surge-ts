/** @jsx dom */
import { dom } from "./renderer";

export class Text {
    tagName: string = "p";
    render() {
        return <this.tagName>text</this.tagName>;
    }
}
export const component = <Missing></Missing>;
