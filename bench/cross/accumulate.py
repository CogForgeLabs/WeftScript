# Map accumulation: 300k updates over 3000 keys.
counts = {}
i = 0
while i < 300000:
    k = f"key_{i % 3000}"
    counts[k] = counts.get(k, 0) + 1
    i += 1
print(counts["key_0"])
