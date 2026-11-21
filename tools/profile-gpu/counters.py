#!/usr/bin/env python3
"""Podsumowanie liczników GPU ze śladu Instruments (profile.sh).

    python3 tools/profile-gpu/counters.py /tmp/gpu-counters.trace

Eksportuje tabele gpu-counter-info i gpu-counter-value (xctrace export, XML z kompresją id/ref),
bierze próbki z zajętością compute > 5% i wypisuje średnią, medianę, p90 i maksimum głównych
liczników: zajętość, limitery i wykorzystanie jednostek ALU, L1, pamięć. Limiter to czas, w którym
praca danego typu czeka na wykonanie (blokuje przebieg); wykorzystanie to czas jej wykonywania.
"""
import collections, os, statistics as st, subprocess, sys, tempfile
import xml.etree.ElementTree as ET

KEY = [
    (3, "Kernel Occupancy"), (53, "Compute SIMD Groups Inflight"),
    (11, "ALU Utilization"), (12, "F32 Limiter"), (13, "F32 Utilization"),
    (16, "Integer and Conditional Limiter"), (17, "Integer and Conditional Utilization"),
    (18, "Control Flow Limiter"), (19, "Control Flow Utilization"),
    (6, "Instruction Throughput Limiter"), (7, "Instruction Throughput Utilization"),
    (22, "L1 Cache Limiter"), (23, "L1 Cache Utilization"), (44, "Buffer L1 Miss Rate"),
    (58, "L1 Read Bandwidth GiB/s"), (74, "GPU Bandwidth GB/s"),
    (85, "Last Level Cache Limiter"), (86, "Last Level Cache Utilization"),
    (54, "Register Pressure Influence"), (56, "Memory Request Stalls Influence"),
]

def export(trace, schema, extra, out):
    xpath = f'/trace-toc/run[@number="1"]/data/table[@schema="{schema}"{extra}]'
    env = dict(os.environ, DEVELOPER_DIR=os.environ.get("DEVELOPER_DIR", "/Applications/Xcode.app/Contents/Developer"))
    with open(out, "w") as f:
        subprocess.run(["xcrun", "xctrace", "export", "--input", trace, "--xpath", xpath], stdout=f, stderr=subprocess.DEVNULL, env=env, check=True)

def rows(path):
    vals = {}
    for _, el in ET.iterparse(path, events=("end",)):
        if el.tag == "row":
            cells = []
            for x in el:
                if "ref" in x.attrib:
                    cells.append(vals[x.attrib["ref"]])
                else:
                    if "id" in x.attrib:
                        vals[x.attrib["id"]] = x.text
                    cells.append(x.text)
            yield cells
            el.clear()

def pct(v, p):
    v = sorted(v)
    return v[int(p * (len(v) - 1))]

def main():
    trace = sys.argv[1]
    tmp = tempfile.mkdtemp()
    export(trace, "gpu-counter-info", ' and @shader-profiler="1"', f"{tmp}/info.xml")
    export(trace, "gpu-counter-value", "", f"{tmp}/value.xml")
    names = {int(c[1]): c[2] for c in rows(f"{tmp}/info.xml")}
    per = collections.defaultdict(list)
    for c in rows(f"{tmp}/value.xml"):
        per[int(c[1])].append(float(c[2]))
    if len(names) < 10:
        sys.exit(f"tylko {len(names)} liczników: nagranie bez sudo? (patrz profile.sh)")
    n = len(per[3])
    busy = [i for i in range(n) if per[3][i] > 5.0]
    print(f"{len(names)} liczników; próbek {n}, z zajętością compute > 5%: {len(busy)}")
    print(f"{'licznik':<38}{'średnia':>9}{'mediana':>9}{'p90':>9}{'max':>9}")
    for cid, label in KEY:
        v = per[cid]
        v = [v[i] for i in busy] if len(v) == n else [x for x in v if x > 0] or [0.0]
        print(f"{label:<38}{st.mean(v):>9.1f}{st.median(v):>9.1f}{pct(v, .9):>9.1f}{max(v):>9.1f}")

main()
