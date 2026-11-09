# Sample sheets

Columns: `sample` (pipeline ID, also the output file names), `run` (SRA accession) or `fastq_1`/`fastq_2` (local files), `condition` (used for analysis), `geo_condition` (label as deposited in GEO).

## GSE197555 A549: GEO labels appear to be swapped

In GEO, GSM5920523-25 ("A549 1-3") are listed as 0.1% DMSO and GSM5920526-28 ("A549 trame 1-3") as 100 nM trametinib for 24 h. The data show the reverse: every classic ERK/MEK target gene (DUSP6, SPRY4, ETV4, ETV5, DUSP4, SPRY2, PHLDA1, EGR1, FOSL1, CCND1) is 3-130x lower in the samples labelled DMSO, which is what 24 h of MEK inhibition does in KRAS-mutant A549 cells.

The H460 samples from the same GEO series, run through the same pipeline, show all ten genes lower in the samples labelled trametinib, as expected. So the pipeline and the expected biology look fine, and the A549 labels appear to be flipped. No correction has been published for this series; this conclusion rests on the data alone.

`condition` in `GSE197555_A549.csv` therefore holds the corrected label, and `geo_condition` keeps the original. The `sample` IDs keep the GEO-based names so the existing pipeline outputs (BAMs, counts) still match. The evidence is reproduced in `analysis/01_differential_expression.qmd`.

## Run commands

Inputs come from `scripts/get_inputs.sh`. Both GSE197555 libraries are unstranded (about 80% of read pairs assigned with `-s 0`, against about 43% with `reverse`).

```bash
nextflow run pipeline/main.nf --samplesheet samplesheets/GSE197555_A549.csv --gtf data/ref/Homo_sapiens.GRCh38.116.chr.gtf --hisat2_index data/ref/grch38_tran --strandedness unstranded --outdir results/GSE197555_A549
nextflow run pipeline/main.nf --samplesheet samplesheets/GSE197555_H460_subset.csv --gtf data/ref/Homo_sapiens.GRCh38.116.chr.gtf --hisat2_index data/ref/grch38_tran --strandedness unstranded --outdir results/GSE197555_H460_subset
```
