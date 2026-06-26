text = "The quick brown fox. The lazy dog! The fox jumps; the dog sleeps."
clean = text.lower().replace(".", " ").replace(",", " ").replace("!", " ").replace(";", " ")
words = [w for w in clean.split(" ") if w != ""]
counts = {}
for w in words:
    counts[w] = counts.get(w, 0) + 1
print("Word frequency analysis")
print(f"Total words: {len(words)}")
print(f"Unique words: {len(counts.keys())}")
print("Counts (alphabetical):")
for w in sorted(counts.keys()):
    print(f"  {w}: {counts[w]}")
best = ""
best_n = 0
for w in sorted(counts.keys()):
    if counts[w] > best_n:
        best_n = counts[w]
        best = w
print(f"Most frequent: {best} ({best_n})")
