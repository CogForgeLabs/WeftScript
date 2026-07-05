// Capability-parity pipeline in idiomatic TypeScript (Node, no dependencies):
// schema validation, constraint check, PARALLEL CPU-bound scoring, PII
// redaction, file output, invariant verification. Node has no parallel map —
// worker_threads plus manual chunking and message plumbing is the standard way
// to use more than one core for CPU-bound work.
import {
    Worker,
    isMainThread,
    parentPort,
    workerData,
} from "node:worker_threads";
import { writeFileSync } from "node:fs";
import { availableParallelism } from "node:os";
import { fileURLToPath } from "node:url";

interface Order {
    id: string;
    amount: number;
    email: string;
}

const EMAIL_RE = /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g;

function make(i: number): Order {
    return { id: `ord-${i}`, amount: (i * 7) % 500, email: `user${i}@corp.com` };
}

function validate(o: unknown): o is Order {
    if (typeof o !== "object" || o === null) return false;
    const r = o as Record<string, unknown>;
    return (
        typeof r.id === "string" &&
        typeof r.amount === "number" &&
        typeof r.email === "string"
    );
}

function nonNegative(o: Order): boolean {
    return o.amount >= 0;
}

function score(o: Order): number {
    let s = 0;
    let j = 0;
    while (j < 200) {
        s += Math.sqrt(o.amount * j + 1);
        j += 1;
    }
    return Math.round(s * 10) / 10;
}

function redact(text: string): string {
    return text.replace(EMAIL_RE, "[REDACTED:email]");
}

function runWorkerChunk(chunk: Order[]): Promise<number[]> {
    return new Promise((resolve, reject) => {
        const worker = new Worker(fileURLToPath(import.meta.url), {
            workerData: chunk,
        });
        worker.once("message", resolve);
        worker.once("error", reject);
    });
}

async function main(): Promise<void> {
    const orders = Array.from({ length: 2000 }, (_, i) => make(i));
    const valid = orders.filter((o) => validate(o) && nonNegative(o));

    const nWorkers = availableParallelism();
    const chunkSize = Math.ceil(valid.length / nWorkers);
    const chunks: Order[][] = [];
    for (let i = 0; i < valid.length; i += chunkSize) {
        chunks.push(valid.slice(i, i + chunkSize));
    }
    const parts = await Promise.all(chunks.map(runWorkerChunk));
    const scores = parts.flat();

    let top = 0;
    for (const s of scores) {
        if (s > top) top = s;
    }

    const report = `orders=${valid.length} top=${top} contact=${valid[0].email}`;
    console.log(redact(report));
    writeFileSync("pipeline_report.txt", redact(report));
    console.log(`invariant holds: ${valid.every((o) => o.amount >= 0)}`);
}

if (isMainThread) {
    main();
} else {
    parentPort!.postMessage((workerData as Order[]).map(score));
}
