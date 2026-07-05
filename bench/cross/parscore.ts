// CPU-bound scoring of 20k items — idiomatic single-thread TypeScript.
// (Parallelism would require worker_threads: separate worker files or blob
// URLs, message passing, and manual chunking.)
function score(x: number): number {
    let s = 0;
    let i = 0;
    while (i < 50) {
        s += Math.sqrt(x * i + 1);
        i += 1;
    }
    return s;
}

const data = Array.from({ length: 20000 }, (_, i) => i);
const scores = data.map(score);
const total = scores.reduce((a, b) => a + b, 0);
console.log((total / 1000000).toFixed(3));
