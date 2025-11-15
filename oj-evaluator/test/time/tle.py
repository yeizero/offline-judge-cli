import time
start = time.process_time()
while time.process_time() - start < 1.0:
    pass
print("end")