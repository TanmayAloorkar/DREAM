"""Assert the test-profile counts match what make_test_data.py simulated."""
import csv
import sys

results = sys.argv[1] if len(sys.argv) > 1 else "results/test"

with open(f"{results}/featurecounts/counts.txt.summary") as f:
    summary = {row[0]: list(map(int, row[1:])) for row in csv.reader(f, delimiter="\t") if row[0] != "Status"}
for i, assigned in enumerate(summary["Assigned"]):
    frac = assigned / sum(v[i] for v in summary.values())
    assert frac > 0.9, f"sample {i}: only {frac:.1%} pairs assigned (wrong --strandedness?)"

with open(f"{results}/featurecounts/counts.txt") as f:
    rows = [r for r in csv.reader(f, delimiter="\t") if not r[0].startswith("#")]
samples = [c.removesuffix(".bam") for c in rows[0][6:]]
counts = {r[0]: dict(zip(samples, map(int, r[6:]))) for r in rows[1:]}


def raw_ratio(gene):
    c = counts[gene]
    return (c["drug_1"] + c["drug_2"]) / (c["ctrl_1"] + c["ctrl_2"])


# Up/down genes shift library composition, so scale by the unchanged genes (as DESeq2 size factors would)
unchanged = sorted(raw_ratio(f"GENE{g:02d}") for g in range(10, 20))
scale = (unchanged[4] + unchanged[5]) / 2


def ratio(gene):
    return raw_ratio(gene) / scale


assert all(3 < ratio(f"GENE{g:02d}") < 5.3 for g in range(5)), "up genes not ~4x up"
assert all(0.19 < ratio(f"GENE{g:02d}") < 0.33 for g in range(5, 10)), "down genes not ~4x down"
assert all(0.8 < ratio(f"GENE{g:02d}") < 1.25 for g in range(10, 20)), "unchanged genes moved"
print("counts OK")
