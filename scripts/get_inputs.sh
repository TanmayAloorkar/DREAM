#!/usr/bin/env bash
# Fetch everything the pipeline and analysis reports need besides the SRA runs
# (the pipeline downloads those itself) and LINCS (scripts/download_lincs.sh).
# Run from the repo root with the dream-qc environment active (for fastq-dump).
# Checksums are those of the files used for the published results; a mismatch
# means the source has been updated since.
set -euo pipefail

get() { # url, output file
  if command -v aria2c >/dev/null; then
    aria2c -x16 -s16 -k1M -c --file-allocation=none -d "$(dirname "$2")" -o "$(basename "$2")" "$1"
  else
    curl -L -C - -o "$2" "$1"
  fi
}

# Reference: prebuilt HISAT2 GRCh38 index with transcripts, and the Ensembl 116 annotation
mkdir -p data/ref
if [ ! -d data/ref/grch38_tran ]; then
  get https://genome-idx.s3.amazonaws.com/hisat/grch38_tran.tar.gz data/ref/grch38_tran.tar.gz
  tar -xzf data/ref/grch38_tran.tar.gz -C data/ref && rm data/ref/grch38_tran.tar.gz
fi
if [ ! -f data/ref/Homo_sapiens.GRCh38.116.chr.gtf ]; then
  get https://ftp.ensembl.org/pub/release-116/gtf/homo_sapiens/Homo_sapiens.GRCh38.116.chr.gtf.gz data/ref/Homo_sapiens.GRCh38.116.chr.gtf.gz
  gunzip data/ref/Homo_sapiens.GRCh38.116.chr.gtf.gz
fi
echo "07fdb1b29e2f1002c0a473bb4ddce3ebb6de1728ef1a539852188ee6ffb19a33  data/ref/Homo_sapiens.GRCh38.116.chr.gtf" | sha256sum -c -

# H460 check samples: first 2M read pairs of each run (used by the label check in analysis/01)
mkdir -p data/h460_subset
for run in SRR18159283 SRR18159282 SRR18159281 SRR18159280 SRR18159278 SRR18159277; do
  [ -f "data/h460_subset/${run}_2.fastq.gz" ] || fastq-dump -X 2000000 --split-files --gzip -O data/h460_subset "$run"
done

# TCGA project from the UCSC Xena GDC hub (analysis/03); set TCGA_PROJECT for another cancer type
project=${TCGA_PROJECT:-TCGA-LUAD}
mkdir -p data/tcga
for f in star_tpm survival clinical somaticmutation_wxs; do
  [ -f "data/tcga/$project.$f.tsv.gz" ] || get "https://gdc-hub.s3.us-east-1.amazonaws.com/download/$project.$f.tsv.gz" "data/tcga/$project.$f.tsv.gz"
done
[ "$project" = TCGA-LUAD ] || exit 0  # checksums below are for the published LUAD results only
sha256sum -c - <<'EOF'
460dab5f84d8fa7da8ef17c913d2816cd66a34e837908e64e0b3c7d65e3d40ee  data/tcga/TCGA-LUAD.star_tpm.tsv.gz
edbf9e416dd6dafa4a4fae9174a7a9e1909bc0b143d9fc80b1962b1172bd2d6c  data/tcga/TCGA-LUAD.survival.tsv.gz
3361a8054624f9126232974b020f388a38e56c7d0bc12e6b9e9fb9d5757845de  data/tcga/TCGA-LUAD.clinical.tsv.gz
9b6145505d4c853ea7ecf4165e3df9bc440e28ec4be4a82509696e049218eae1  data/tcga/TCGA-LUAD.somaticmutation_wxs.tsv.gz
EOF
