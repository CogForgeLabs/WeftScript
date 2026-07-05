# Dot product of two 2M-element vectors — Python with numpy (external library).
import numpy as np

n = 2000000
a = np.arange(n, dtype=np.float64)
print(round(float(np.dot(a, a)) / 1e15, 3))
