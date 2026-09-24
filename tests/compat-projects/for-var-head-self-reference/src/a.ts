for (var key in key) { }
for (var element of element) { }
function scoped() {
    for (var inner in inner) { }
    for (var item of item) { }
}
var declaredFirst = "a";
for (var declaredFirst in declaredFirst) { }
