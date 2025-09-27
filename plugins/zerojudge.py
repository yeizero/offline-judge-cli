import requests
from bs4 import BeautifulSoup
import re

def ask(content):
    return input(f"/ask {content}\n")

print("/info ZeroJudge範例測資抓取")

def fetch_valid_soup():
    while True:
        url = ask("題目連結或題號:")
        if is_id := len(url.strip()) == 4:
            url = f"https://zerojudge.tw/ShowProblem?problemid={url.strip()}"
        try:
            r = requests.get(url)
            if r.status_code != 200:
                print("/error 無法連線，HTTP", r.status_code)
                continue
            return BeautifulSoup(r.text, 'html.parser')
        except Exception as _:
            print(f"/error 請輸入有效{'題號' if is_id else '連結'}")
            continue

soup = fetch_valid_soup()

inputs = []
outputs = []
memory_limit = None
time_limit = None

# 抓取範例輸入與範例輸出
for panel in soup.select('.panel'):
    heading = panel.select_one('.panel-heading')
    body = panel.select_one('.panel-body pre')
    if not heading or not body:
        continue
    text = heading.get_text()
    content = body.get_text().strip('\n')
    if "範例輸入" in text:
        inputs.append(content)
    elif "範例輸出" in text:
        outputs.append(content)

# 抓取記憶體限制與時間限制
limit_panel = soup.select_one('.col-md-3 .panel-body')
if limit_panel:
    text = limit_panel.get_text(separator="\n")

    # E.G. "記憶體限制： 512 MB"
    mem_match = re.search(r"記憶體限制：\s*([\d.]+)\s*(MB|KB)", text)
    if mem_match:
        mem_val, unit = mem_match.groups()
        memory_limit = float(mem_val) * 1024 if unit == "MB" else float(mem_val)

    # E.G. "1.0s"
    time_matches = re.findall(r"(\d+(?:\.\d+)?)\s*s", text)
    if time_matches:
        time_limit = max(float(t) for t in time_matches) * 1000

print("/result")
for i in range(min(len(inputs), len(outputs))):
    in_lines = inputs[i].strip().splitlines()
    out_lines = outputs[i].strip().splitlines()

    print(f"input {len(in_lines)}")
    for line in in_lines:
        print(line)

    print(f"answer {len(out_lines)}")
    for line in out_lines:
        print(line)

if memory_limit is not None and time_limit is not None:
    print("limit 1")
    print(f"memory {int(memory_limit)} time {int(time_limit)}")
