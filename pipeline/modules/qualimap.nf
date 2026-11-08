process QUALIMAP {
    tag "$meta.id"
    conda "${projectDir}/../envs/align.yml"
    cpus 2
    memory '6 GB'
    publishDir "${params.outdir}/qualimap", mode: 'copy'

    input:
    tuple val(meta), path(bam), path(bai)
    path gtf

    output:
    path "${meta.id}"

    script:
    def strand = [unstranded: 'non-strand-specific', forward: 'strand-specific-forward', reverse: 'strand-specific-reverse'][params.strandedness]
    def pe = meta.single_end ? '' : '-pe'
    """
    unset DISPLAY
    qualimap rnaseq -bam $bam -gtf $gtf -outdir ${meta.id} -p $strand $pe --java-mem-size=5G
    """
}
