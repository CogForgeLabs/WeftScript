// Dot product of two 2M-element vectors — idiomatic TypeScript.
const n = 2000000;
const a = new Float64Array(n);
for (let i = 0; i < n; i++) a[i] = i;
let total = 0;
for (let i = 0; i < n; i++) total += a[i] * a[i];
console.log((total / 1e15).toFixed(3));
