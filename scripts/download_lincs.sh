#!/usr/bin/env bash
# Download the LINCS 2020 reference used by analysis/04_drug_repurposing.qmd.
# Same file signatureSearch fetches as ExperimentHub EH7297 ("lincs2"), pulled
# with parallel connections because a single stream can be very slow.
# The small WTCS null distribution (EH3234, 18 KB) is fetched by ExperimentHub on first use.
set -euo pipefail

out=${1:-data/lincs}
url=https://mghp.osn.xsede.org/bir190004-bucket01/ExperimentHub/signatureSearchData/v0.2/lincs2020.h5
sha256=880131f739f34f7d6e6ee1b26d00b11650cb2c96b75242a90cfddc3027b4af08

mkdir -p "$out"
if command -v aria2c >/dev/null; then
  aria2c -x16 -s16 -k1M -c --file-allocation=none -d "$out" -o lincs2020.h5 "$url"
else
  curl -L -C - -o "$out/lincs2020.h5" "$url"
fi
echo "$sha256  $out/lincs2020.h5" | sha256sum -c -
