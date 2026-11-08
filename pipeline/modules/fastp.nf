process FASTP {
    tag "$meta.id"
    conda "${projectDir}/../envs/qc.yml"
    cpus 4
    publishDir "${params.outdir}/fastp", mode: 'copy', pattern: '*.{json,html}'

    input:
    tuple val(meta), path(reads)

    output:
    tuple val(meta), path("${meta.id}_trim*.fastq.gz"), emit: reads
    path "${meta.id}.fastp.json", emit: json
    path "${meta.id}.fastp.html"

    script:
    def io = meta.single_end
        ? "-i $reads -o ${meta.id}_trim.fastq.gz"
        : "-i ${reads[0]} -I ${reads[1]} -o ${meta.id}_trim_1.fastq.gz -O ${meta.id}_trim_2.fastq.gz --detect_adapter_for_pe"
    """
    fastp $io --thread $task.cpus --json ${meta.id}.fastp.json --html ${meta.id}.fastp.html
    """
}
