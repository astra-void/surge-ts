export function outer() {
    function Detector() {}
    const helpers: { run?: () => boolean } = {};
    helpers.run = function () {
        return Detector.isSmall(1);
    };
    helpers.missing = function () {
        return Detector.isSmall(unknownWidth);
    };
    if (typeof globalThis !== "undefined") {
        Detector.isSmall = function (width: number) {
            return width < 10;
        };
    }
    return [Detector, helpers];
}
