process HISAT2_BUILD {
    conda "${projectDir}/../envs/align.yml"
    cpus 4

    input:
    path fasta

    output:
    path "index"

    script:
    """
    mkdir index
    hisat2-build -p $task.cpus $fasta index/genome
    """
}
