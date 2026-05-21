import { useEffect, useState } from "react";

const [value, setValue] = useState(1);
useEffect(() => {
  setValue(value + 1);
}, []);
