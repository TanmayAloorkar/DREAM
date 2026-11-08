process HISAT2_ALIGN {
    tag "$meta.id"
    conda "${projectDir}/../envs/align.yml"
    cpus 16
    // ~8 GB index + 2 sort threads x 768M
    memory '11 GB'
    publishDir "${params.outdir}/hisat2", mode: 'copy'

    input:
    tuple val(meta), path(reads)
    path index

    output:
    tuple val(meta), path("${meta.id}.bam"), path("${meta.id}.bam.bai"), emit: bam
    path "${meta.id}.hisat2.log", emit: log

    script:
    def input = meta.single_end ? "-U $reads" : "-1 ${reads[0]} -2 ${reads[1]}"
    """
    # index dir may hold any prefix (genome, genome_tran, ...)
    INDEX=\$(find -L $index -name '*.1.ht2' | sed 's/\\.1\\.ht2\$//')
    hisat2 -x \$INDEX $input -p $task.cpus --new-summary --summary-file ${meta.id}.hisat2.log \\
        | samtools sort -@ 2 -m 768M -o ${meta.id}.bam -
    samtools index ${meta.id}.bam
    """
}
