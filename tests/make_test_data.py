"""Simulate a tiny genome, annotation and reverse-stranded paired-end reads for pipeline tests.

Drug samples have GENE00-04 up 4x and GENE05-09 down 4x relative to control.
"""
import gzip
import io
import random
from pathlib import Path

random.seed(42)
out = Path(__file__).parent / "data"
out.mkdir(exist_ok=True)
comp = str.maketrans("ACGT", "TGCA")


def rc(seq):
    return seq.translate(comp)[::-1]


genome = "".join(random.choice("ACGT") for _ in range(200_000))
(out / "genome.fa").write_text(
    ">chrT\n" + "\n".join(genome[i:i + 60] for i in range(0, len(genome), 60)) + "\n")

transcripts, gtf = [], []
for g in range(20):
    gid = f"GENE{g:02d}"
    strand = "+" if g % 2 == 0 else "-"
    start = 5_000 + g * 9_000
    exons = [(start + k * 2_000, start + k * 2_000 + 400) for k in range(3)]  # 0-based half-open
    for s, e in exons:
        attrs = f'gene_id "{gid}"; transcript_id "{gid}.1"; gene_name "{gid}";'
        gtf.append(f"chrT\ttest\texon\t{s + 1}\t{e}\t.\t{strand}\t.\t{attrs}")
    tx = "".join(genome[s:e] for s, e in exons)
    transcripts.append(tx if strand == "+" else rc(tx))
(out / "genes.gtf").write_text("\n".join(gtf) + "\n")


def fastq_writer(path):
    # mtime=0 keeps the gzip bytes identical across reruns
    return io.TextIOWrapper(gzip.GzipFile(path, "wb", mtime=0))


for name, fold in {"ctrl_1": 1, "ctrl_2": 1, "drug_1": 4, "drug_2": 4}.items():
    weights = [fold if g < 5 else 1 / fold if g < 10 else 1 for g in range(20)]
    with fastq_writer(out / f"{name}_R1.fastq.gz") as r1, fastq_writer(out / f"{name}_R2.fastq.gz") as r2:
        for i in range(10_000):
            tx = random.choices(transcripts, weights)[0]
            p = random.randrange(len(tx) - 250)
            frag = tx[p:p + 250]
            # dUTP library: R1 is antisense to the transcript, R2 is sense
            for fh, seq in ((r1, rc(frag)[:100]), (r2, frag[:100])):
                fh.write(f"@{name}.{i}\n{seq}\n+\n{'I' * 100}\n")
