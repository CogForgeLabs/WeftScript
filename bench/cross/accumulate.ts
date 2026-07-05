// Map accumulation: 300k updates over 3000 keys.
const counts: Map<string, number> = new Map();
let i = 0;
while (i < 300000) {
    const k = `key_${i % 3000}`;
    counts.set(k, (counts.get(k) ?? 0) + 1);
    i += 1;
}
console.log(counts.get("key_0"));
