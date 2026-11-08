process SRA_DOWNLOAD {
    tag "$sample"
    conda "${projectDir}/../envs/qc.yml"
    cpus 4
    maxForks 2

    input:
    tuple val(sample), val(run)

    output:
    tuple val(sample), path("${run}*.fastq.gz")

    script:
    """
    prefetch $run --max-size 100G
    fasterq-dump $run --split-3 --threads $task.cpus --temp .
    # --split-3 puts unpaired leftovers in ${run}.fastq; drop them for paired runs
    if [ -f ${run}_1.fastq ]; then rm -f ${run}.fastq; fi
    rm -rf $run
    pigz -p $task.cpus ${run}*.fastq
    """
}
