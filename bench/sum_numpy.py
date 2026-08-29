# NumPy does not loop: it hands an array to C. Chunked so a big N does not
# need gigabytes of memory at once.
import sys
import numpy as np
n = int(sys.argv[1])
total = np.int64(0)
step = 10_000_000
for start in range(1, n + 1, step):
    stop = min(start + step - 1, n)
    total += np.arange(start, stop + 1, dtype=np.int64).sum()
print(total)
