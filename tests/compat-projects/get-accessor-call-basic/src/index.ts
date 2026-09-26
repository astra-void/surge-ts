class Gauge {
  get level() {
    return 1;
  }
  set level(value: number) {}
  get label() {
    return "gauge";
  }
  set target(value: number) {}
  reset() {
    return 0;
  }
}

class Meter extends Gauge {}

class Fixed extends Gauge {
  label = "fixed";
}

const gauge = new Gauge();
gauge.level();
gauge.label();
gauge.level(1);
gauge.target();
gauge.reset();
new Meter().level();
new Fixed().label();
