process FEATURECOUNTS {
    conda "${projectDir}/../envs/align.yml"
    cpus 8
    publishDir "${params.outdir}/featurecounts", mode: 'copy'

    input:
    path bams
    path gtf
    val single_end

    output:
    path "counts.txt", emit: counts
    path "counts.txt.summary", emit: summary

    script:
    def strand = [unstranded: 0, forward: 1, reverse: 2][params.strandedness]
    def pe = single_end ? '' : '-p --countReadPairs'
    """
    featureCounts -T $task.cpus -s $strand $pe -a $gtf -o counts.txt $bams
    """
}
