declare const box: { value: number; inner?: { deep: string } } | undefined; const a = box ? box.value : 0; const b = box?.value; const c = box!.value; const d = box?.inner?.deep;
