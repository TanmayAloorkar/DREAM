// RNA-seq: SRA or local FASTQ -> fastp -> HISAT2 -> featureCounts, with FastQC/Qualimap/MultiQC QC.
// Samplesheet columns: sample,run,fastq_1,fastq_2 (set run for SRA download, or fastq_1[/fastq_2] for local files).
// Run from the repo root: local FASTQ paths resolve against the launch directory.

include { SRA_DOWNLOAD  } from './modules/sra_download'
include { FASTQC        } from './modules/fastqc'
include { FASTP         } from './modules/fastp'
include { HISAT2_BUILD  } from './modules/hisat2_build'
include { HISAT2_ALIGN  } from './modules/hisat2_align'
include { FEATURECOUNTS } from './modules/featurecounts'
include { QUALIMAP      } from './modules/qualimap'
include { MULTIQC       } from './modules/multiqc'

workflow {
    if (!params.samplesheet) error "Set --samplesheet"
    if (!params.gtf) error "Set --gtf (uncompressed GTF)"
    if (!params.hisat2_index && !params.fasta) error "Set --hisat2_index or --fasta"
    if (!(params.strandedness in ['unstranded', 'forward', 'reverse'])) error "--strandedness must be unstranded, forward or reverse"

    rows = channel.fromPath(params.samplesheet, checkIfExists: true).splitCsv(header: true)

    local = rows
        .filter { row -> row.fastq_1 }
        .map { row -> tuple(row.sample, [row.fastq_1, row.fastq_2].findAll().collect { f -> file(f, checkIfExists: true) }) }
    remote = SRA_DOWNLOAD(rows.filter { row -> !row.fastq_1 }.map { row -> tuple(row.sample, row.run) })

    reads = local.mix(remote).map { sample, fq ->
        def files = fq instanceof List ? fq : [fq]
        tuple([id: sample, single_end: files.size() == 1], files)
    }

    index = params.hisat2_index
        ? channel.value(file(params.hisat2_index, checkIfExists: true))
        : HISAT2_BUILD(file(params.fasta, checkIfExists: true))
    gtf = file(params.gtf, checkIfExists: true)

    FASTQC(reads)
    FASTP(reads)
    HISAT2_ALIGN(FASTP.out.reads, index)
    QUALIMAP(HISAT2_ALIGN.out.bam, gtf)
    FEATURECOUNTS(
        HISAT2_ALIGN.out.bam.map { meta, bam, bai -> bam }.collect(),
        gtf,
        HISAT2_ALIGN.out.bam.map { meta, bam, bai -> meta.single_end }.first()
    )
    MULTIQC(
        FASTQC.out
            .mix(FASTP.out.json, HISAT2_ALIGN.out.log, QUALIMAP.out, FEATURECOUNTS.out.summary)
            .collect()
    )
}
