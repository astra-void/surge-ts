declare namespace JSX {
    interface Element {}
}

export class Text {
    private tagName = "div";
    render() {
        return <this.tagName>hello</this.tagName>;
    }
}

export const plain = <span />;
