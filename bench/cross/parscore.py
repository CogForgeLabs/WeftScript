# CPU-bound scoring of 20k items — idiomatic single-thread Python.
# (Threads would not help: the GIL serializes pure-Python compute. Using
# multiprocessing requires a __main__ guard, pickling, and pool management.)
import math


def score(x):
    s = 0.0
    i = 0
    while i < 50:
        s += math.sqrt(x * i + 1)
        i += 1
    return s


data = range(20000)
scores = [score(x) for x in data]
print(round(sum(scores) / 1000000, 3))
