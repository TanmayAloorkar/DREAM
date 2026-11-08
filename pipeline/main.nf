nextflow.enable.dsl = 2

include { FASTQC } from './modules/fastqc'

workflow {
    reads = channel
        .fromPath(params.samplesheet)
        .splitCsv(header: true)
        .map { row -> tuple(row.sample, [file(row.fastq_1), file(row.fastq_2)]) }

    FASTQC(reads)
}
