# import psutil
# import os
import gc

gc.disable()

# def get_memory_usage():
#     process = psutil.Process(os.getpid())
#     mem_info = process.memory_info()
#     return mem_info.rss / (1024 * 1024)

# initial_memory = get_memory_usage()
# print(f"Initial memory usage: {initial_memory:.2f} MB")

big_data = []
for i in range(4_000_000):
    big_data.append([i] * 100)

# final_memory = get_memory_usage()
# print(f"Final memory usage: {final_memory:.2f} MB")

# print(f"Memory used: {final_memory - initial_memory:.2f} MB")

print("end")