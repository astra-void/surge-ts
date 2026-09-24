function shadowF() {
  {
    let x = 1;
    { var x = 2; }
  }
  for (let i = 0; i < 1; i++) { var i = 3; }
  {
    const y = 1;
    var y = 2;
  }
  {
    let z = 1;
    { var z; }
  }
}
function shadowG() {
  switch (1) { case 1: let s = 1; { var s = 2; } }
  {
    let t = 1;
    function inner() { var t = 2; }
  }
}
