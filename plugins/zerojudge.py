import requests
from bs4 import BeautifulSoup
import re
from json import dumps

def call(command: str, content = None):
    print(f"{command} {dumps(content)}")

def ask(content: str):
    call("/ask", {"message": content})
    try:
        return input()
    except EOFError:
        exit()

call("/info", "ZeroJudge範例測資抓取")

def fetch_valid_soup():
    while True:
        url = ask("題目連結或題號:")
        if is_id := len(url.strip()) == 4:
            url = f"https://zerojudge.tw/ShowProblem?problemid={url.strip()}"
        try:
            r = requests.get(url)
            if r.status_code != 200:
                call("/error", f"無法連線，HTTP {r.status_code}")
                continue
            return BeautifulSoup(r.text, 'html.parser')
        except Exception as _:
            call("/error", f"請輸入有效{'題號' if is_id else '連結'}")
            continue

soup = fetch_valid_soup()

inputs: list[str] = []
answers: list[str] = []
memory_limit: float | None = None
time_limit: float | None = None

# 抓取範例輸入與範例輸出
for panel in soup.select('.panel'):
    heading = panel.select_one('.panel-heading')
    body = panel.select_one('.panel-body pre')
    if not heading or not body:
        continue
    text = heading.get_text()
    content = body.get_text().strip('\n')
    if "範例輸入" in text:
        inputs.append(re.sub("\r", "", content))
    elif "範例輸出" in text:
        answers.append(re.sub("\r", "", content))

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

metadata: dict[str, str] = {}

title = soup.select_one('#problem_title')
if title:
    title = title.parent
    if title:
        metadata["title"] = re.sub(r"[\n\t]+", " ", title.get_text().strip())

description = soup.select_one("#problem_content")
if description:
    metadata["description"] = description.get_text("\n").strip()

    input_description = soup.select_one("#problem_theinput")
    if input_description:
        parent = input_description.parent
        grandparent = parent.parent if parent else None
        heading = grandparent.select_one(".panel-heading") if grandparent else None
        if heading:
            metadata["description"] += "\n\n" + heading.get_text() + "\n"
        metadata["description"] += "\n" + input_description.get_text("\n").strip()

    output_description = soup.select_one("#problem_theoutput")
    if output_description:
        parent = output_description.parent
        grandparent = parent.parent if parent else None
        heading = grandparent.select_one(".panel-heading") if grandparent else None
        if heading:
            metadata["description"] += "\n\n" + heading.get_text() + "\n"
        metadata["description"] += "\n" + output_description.get_text("\n").strip()
    
    metadata["description"] = re.sub(r"[\r\t]+", "", metadata["description"])
    metadata["description"] = re.sub(r"([^\S\n]*\n){4}[^\S\n]*", "\n\n", metadata["description"])

def float_to_int(value: float | None) -> int | None:
    if value is None:
        return None
    return int(value)

call("/data", {
    "limit": {
        "memory": float_to_int(memory_limit),
        "time": float_to_int(time_limit)
    },
    "cases": [{"input": input, "answer": answer} for input, answer in zip(inputs, answers)],
    "meta": metadata
})
