import sys

for line in sys.stdin:
    try:
        a, b = map(int, line.strip().split())
        print(a + b)
    except ValueError:
        continue