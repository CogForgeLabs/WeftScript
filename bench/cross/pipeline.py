# Capability-parity pipeline in idiomatic Python (standard library only):
# schema validation, constraint check, PARALLEL CPU-bound scoring, PII
# redaction, file output, invariant verification. Each capability that Weft
# ships built-in must be assembled by hand here.
import math
import re
from concurrent.futures import ProcessPoolExecutor

SCHEMA = {"id": str, "amount": (int, float), "email": str}

EMAIL_RE = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")


def make(i):
    return {"id": f"ord-{i}", "amount": i * 7 % 500, "email": f"user{i}@corp.com"}


def validate(rec):
    for field, ty in SCHEMA.items():
        if field not in rec or not isinstance(rec[field], ty):
            return False
    return True


def non_negative(rec):
    return rec["amount"] >= 0


def score(o):
    s = 0.0
    j = 0
    while j < 200:
        s += math.sqrt(o["amount"] * j + 1)
        j += 1
    return round(s, 1)


def redact(text):
    return EMAIL_RE.sub("[REDACTED:email]", text)


def main():
    orders = [make(i) for i in range(2000)]
    valid = [o for o in orders if validate(o) and non_negative(o)]

    # Threads do not help: the GIL serializes pure-Python compute. Real
    # parallelism needs process pools (pickling + spawn overhead + __main__
    # guard required on Windows).
    with ProcessPoolExecutor() as pool:
        scores = list(pool.map(score, valid, chunksize=64))

    top = 0.0
    for s in scores:
        if s > top:
            top = s

    report = f"orders={len(valid)} top={top} contact={valid[0]['email']}"
    print(redact(report))
    with open("pipeline_report.txt", "w") as f:
        f.write(redact(report))
    print(f"invariant holds: {str(all(o['amount'] >= 0 for o in valid)).lower()}")


if __name__ == "__main__":
    main()
