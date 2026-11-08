process FASTQC {
    tag "$sample"
    conda "${projectDir}/../envs/qc.yml"
    publishDir "${params.outdir}/fastqc", mode: 'copy'

    input:
    tuple val(sample), path(reads)

    output:
    path "*_fastqc.{zip,html}"

    script:
    """
    fastqc --threads $task.cpus --quiet $reads
    """
}
