class Panel {
    props!: { heading: string };
    render() { return null; }
}

const panelOk = <Panel heading="h" />;
const panelMissing = <Panel />;
const panelWrong = <Panel heading={1} />;
