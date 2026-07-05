# Dot product of two 2M-element vectors — pure Python (no libraries).
n = 2000000
a = list(range(n))
total = 0.0
for i in range(n):
    total += a[i] * a[i]
print(round(total / 1e15, 3))
