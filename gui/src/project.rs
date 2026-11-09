//! Everything the GUI knows about a DREAM checkout: where it is, what is installed,
//! which inputs exist, the commands for each step and the reports it has produced.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const LINCS_BYTES: u64 = 6_263_924_772;
const GTF: &str = "data/ref/Homo_sapiens.GRCh38.116.chr.gtf";
const INDEX: &str = "data/ref/grch38_tran";
const H460_RUNS: [&str; 6] = ["SRR18159283", "SRR18159282", "SRR18159281", "SRR18159280", "SRR18159278", "SRR18159277"];
const TCGA_FILES: [&str; 4] = ["star_tpm", "survival", "clinical", "somaticmutation_wxs"];

pub fn is_dream(dir: &Path) -> bool {
    dir.join("pipeline/main.nf").is_file() && dir.join("analysis").is_dir()
}

fn setting_path(name: &str) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| home().map(|h| h.join(".config")))?;
    Some(base.join("dream-gui").join(name))
}

/// A small saved setting (one line in ~/.config/dream-gui/<name>).
pub fn setting(name: &str) -> Option<String> {
    setting_path(name).and_then(|p| fs::read_to_string(p).ok()).map(|s| s.trim().to_string())
}

pub fn save_setting(name: &str, value: &str) {
    if let Some(p) = setting_path(name) {
        let _ = fs::create_dir_all(p.parent().unwrap());
        let _ = fs::write(p, value);
    }
}

/// The DREAM folder the executable sits in (Run_Dream in a release, or gui/target/... in a checkout)
/// comes first, so a fresh download never opens an older saved folder; then the saved choice,
/// then the working directory.
pub fn find() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok();
    let saved = setting("project").map(PathBuf::from);
    let cwd = std::env::current_dir().ok();
    exe.iter()
        .flat_map(|e| e.ancestors().skip(1).map(Path::to_path_buf))
        .chain(saved)
        .chain(cwd)
        .find(|d| is_dream(d))
}

pub fn remember(dir: &Path) {
    save_setting("project", &dir.to_string_lossy());
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub fn tilde(path: &Path) -> String {
    match home() {
        Some(h) if path.starts_with(&h) => format!("~/{}", path.strip_prefix(&h).unwrap().display()),
        _ => path.display().to_string(),
    }
}

fn on_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(name)).find(|p| p.is_file())
}

/// Conda install root: $CONDA_EXE if set, otherwise the usual installer locations.
fn conda_root() -> Option<PathBuf> {
    if let Some(exe) = std::env::var_os("CONDA_EXE") {
        return PathBuf::from(exe).parent()?.parent().map(Path::to_path_buf);
    }
    let h = home()?;
    ["miniforge3", "mambaforge", "miniconda3", "anaconda3"].iter().map(|d| h.join(d)).find(|d| d.join("bin/conda").is_file())
}

pub fn size_text(bytes: u64) -> String {
    let gb = bytes as f64 / 1e9;
    if gb >= 1.0 { format!("{gb:.1} GB") } else { format!("{:.0} MB", bytes as f64 / 1e6) }
}

pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[derive(Clone, Copy, PartialEq)]
pub enum State {
    Ok,
    Missing,
    Partial,
}

pub struct Check {
    pub label: &'static str,
    pub detail: String,
    pub state: State,
}

impl Check {
    fn new(label: &'static str, ok: bool, detail: impl Into<String>) -> Self {
        Check { label, detail: detail.into(), state: if ok { State::Ok } else { State::Missing } }
    }
}

pub struct Status {
    pub conda: Option<PathBuf>,
    pub envs: Vec<Check>,
    pub inputs: Vec<Check>,
    pub lincs: Check,
}

impl Status {
    pub fn read(root: &Path, tcga_project: &str) -> Self {
        let conda = conda_root();
        let env = |name: &str| conda.as_ref().map(|c| c.join("envs").join(name)).filter(|p| p.is_dir());
        let nextflow = on_path("nextflow");
        let java = on_path("java");
        let envs = vec![
            Check::new("Conda", conda.is_some(), conda.as_deref().map(tilde).unwrap_or("Miniforge not found".into())),
            Check::new("dream-qc environment", env("dream-qc").is_some(), "SRA tools and QC, used to fetch inputs"),
            Check::new("dream-r environment", env("dream-r").is_some(), "R, Bioconductor and Quarto, used for the reports"),
            Check::new("Nextflow", nextflow.is_some(), nextflow.as_deref().map(tilde).unwrap_or("not on PATH".into())),
            Check::new("Java", java.is_some(), java.as_deref().map(tilde).unwrap_or("not on PATH".into())),
        ];

        let index_ok = fs::read_dir(root.join(INDEX))
            .map(|d| d.flatten().any(|e| e.file_name().to_string_lossy().ends_with(".1.ht2")))
            .unwrap_or(false);
        let gtf = fs::metadata(root.join(GTF)).map(|m| m.len()).ok();
        let h460 = H460_RUNS
            .iter()
            .flat_map(|r| [1, 2].map(|n| root.join(format!("data/h460_subset/{r}_{n}.fastq.gz"))))
            .filter(|p| p.is_file())
            .count();
        let tcga = TCGA_FILES.iter().filter(|f| tcga_file(root, tcga_project, f).is_file()).count();
        let inputs = vec![
            Check::new("HISAT2 index, GRCh38", index_ok, INDEX),
            Check::new("Ensembl 116 annotation", gtf.is_some(), gtf.map(size_text).unwrap_or(GTF.into())),
            Check { label: "H460 check samples", detail: format!("{h460} of 12 FASTQ files"), state: count_state(h460, 12) },
            Check { label: "TCGA data", detail: format!("{tcga_project}: {tcga} of 4 files"), state: count_state(tcga, 4) },
        ];

        let lincs_len = fs::metadata(root.join("data/lincs/lincs2020.h5")).map(|m| m.len()).unwrap_or(0);
        let lincs = Check {
            label: "LINCS 2020 signatures",
            detail: if lincs_len == 0 {
                "data/lincs/lincs2020.h5, 6.3 GB".into()
            } else {
                format!("{} of {}", size_text(lincs_len), size_text(LINCS_BYTES))
            },
            state: if lincs_len == LINCS_BYTES { State::Ok } else if lincs_len > 0 { State::Partial } else { State::Missing },
        };
        Status { conda, envs, inputs, lincs }
    }

    pub fn ok(&self, label: &str) -> bool {
        self.envs.iter().chain(&self.inputs).chain([&self.lincs]).any(|c| c.label == label && c.state == State::Ok)
    }

    pub fn conda_bin(&self, tool: &str) -> String {
        let root = self.conda.clone().unwrap_or_default();
        let mamba = root.join("bin/mamba");
        let path = if tool == "mamba" && !mamba.is_file() { root.join("bin/conda") } else { root.join("bin").join(tool) };
        quote(&path.to_string_lossy())
    }
}

fn count_state(found: usize, want: usize) -> State {
    match found {
        0 => State::Missing,
        n if n == want => State::Ok,
        _ => State::Partial,
    }
}

pub fn tcga_file(root: &Path, project: &str, kind: &str) -> PathBuf {
    root.join(format!("data/tcga/{project}.{kind}.tsv.gz"))
}

/// Free and total memory in bytes from /proc/meminfo.
pub fn memory() -> Option<(u64, u64)> {
    let text = fs::read_to_string("/proc/meminfo").ok()?;
    let field = |key: &str| {
        text.lines().find(|l| l.starts_with(key))?.split_whitespace().nth(1)?.parse::<u64>().ok().map(|kb| kb * 1024)
    };
    Some((field("MemAvailable:")?, field("MemTotal:")?))
}

pub struct Report {
    pub title: String,
    pub path: PathBuf,
    pub modified: Option<SystemTime>,
}

pub struct ReportGroup {
    pub name: &'static str,
    pub reports: Vec<Report>,
}

fn title_for(stem: &str) -> String {
    match stem {
        "01_differential_expression" => "01 · Differential expression and pathway enrichment".into(),
        "02_mechanism" => "02 · Pathway and transcription factor activity".into(),
        "03_patients" => "03 · Validation in TCGA patients".into(),
        "04_drug_repurposing" => "04 · Drug repurposing with LINCS 2020".into(),
        "index" => "Report index".into(),
        "multiqc_report" => "MultiQC".into(),
        s => s.replace('_', " "),
    }
}

fn html_in(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "html")).collect())
        .unwrap_or_default();
    files.sort();
    files
}

fn report(path: PathBuf, title: String) -> Report {
    let modified = fs::metadata(&path).and_then(|m| m.modified()).ok();
    Report { title, path, modified }
}

pub fn reports(root: &Path) -> Vec<ReportGroup> {
    let stem = |p: &Path| p.file_stem().unwrap().to_string_lossy().to_string();
    let latest = html_in(&root.join("analysis")).into_iter().map(|p| report(p.clone(), title_for(&stem(&p)))).collect();
    let published = html_in(&root.join("docs"))
        .into_iter()
        .filter(|p| !stem(p).starts_with("multiqc"))
        .map(|p| report(p.clone(), title_for(&stem(&p))))
        .collect();
    let mut qc: Vec<Report> = fs::read_dir(root.join("results"))
        .map(|d| d.flatten().map(|e| e.path().join("multiqc/multiqc_report.html")).filter(|p| p.is_file()).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let run = p.ancestors().nth(2).map(|d| stem(d)).unwrap_or_default();
            report(p, format!("MultiQC · {run}"))
        })
        .collect();
    qc.sort_by(|a, b| a.title.cmp(&b.title));
    vec![
        ReportGroup { name: "Latest renders", reports: latest },
        ReportGroup { name: "Published copies (docs/)", reports: published },
        ReportGroup { name: "Pipeline quality control", reports: qc },
    ]
}

pub struct PipelineArgs<'a> {
    pub samplesheet: &'a str,
    pub strandedness: &'a str,
    pub outdir: &'a str,
    pub resume: bool,
}

pub fn pipeline_command(a: &PipelineArgs) -> String {
    format!(
        "nextflow run pipeline/main.nf --samplesheet {} --gtf {GTF} --hisat2_index {INDEX} --strandedness {} --outdir {} -ansi-log false{}",
        quote(a.samplesheet),
        a.strandedness,
        quote(a.outdir),
        if a.resume { " -resume" } else { "" }
    )
}

pub struct ReportArgs<'a> {
    pub root: &'a Path,
    pub samplesheet: &'a str,
    pub outdir: &'a str,
    pub treatment: &'a str,
    pub control: &'a str,
    pub moa: &'a str,
    pub tcga_project: &'a str,
    pub label_check: bool,
    pub gates: bool,
    pub selected: [bool; 4],
}

pub const REPORTS: [&str; 4] = ["01_differential_expression", "02_mechanism", "03_patients", "04_drug_repurposing"];

/// One quarto render per selected report, chained so the first failure stops the rest.
/// Paths are absolute because Quarto runs from analysis/.
pub fn report_command(a: &ReportArgs, conda: &str) -> String {
    let abs = |p: &str| {
        let p = Path::new(p);
        if p.is_absolute() { p.to_path_buf() } else { a.root.join(p) }
    };
    let out = abs(a.outdir);
    let sheet = abs(a.samplesheet);
    let at = |sub: &str| out.join(sub).to_string_lossy().to_string();
    let deseq2 = at("deseq2/deseq2_results.tsv");
    let tcga_dir = format!("tcga_{}", a.tcga_project.trim_start_matches("TCGA-").to_lowercase());
    let sheet = sheet.to_string_lossy().to_string();
    let params: [Vec<(&str, String)>; 4] = [
        vec![
            ("counts", at("featurecounts/counts.txt")),
            ("samplesheet", sheet.clone()),
            ("outdir", at("deseq2")),
            ("treatment", a.treatment.into()),
            ("control", a.control.into()),
            ("label_check", a.label_check.to_string()),
        ],
        vec![
            ("deseq2", deseq2.clone()),
            ("counts", at("deseq2/counts_matrix.tsv.gz")),
            ("samplesheet", sheet),
            ("outdir", at("decoupler")),
            ("treatment", a.treatment.into()),
            ("control", a.control.into()),
        ],
        vec![("deseq2", deseq2.clone()), ("tcga_project", a.tcga_project.into()), ("outdir", at(&tcga_dir))],
        vec![
            ("deseq2", deseq2),
            ("treatment", a.treatment.into()),
            ("positive_control_moa", a.moa.into()),
            ("outdir", at("lincs")),
        ],
    ];
    let mut steps = vec!["cd analysis".to_string()];
    for (i, name) in REPORTS.iter().enumerate().filter(|(i, _)| a.selected[*i]) {
        let mut p = params[i].clone();
        if !a.gates {
            match i {
                0 | 1 => p.push(("gate_direction", "none".into())),
                2 => p.push(("gate_pathway", "none".into())),
                _ => {}
            }
        }
        let flags: Vec<String> = p.iter().map(|(k, v)| format!("-P {}", quote(&format!("{k}:{v}")))).collect();
        steps.push(format!("{conda} run -n dream-r --no-capture-output quarto render {name}.qmd {}", flags.join(" ")));
    }
    steps.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_single_quotes_for_bash() {
        assert_eq!(quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn report_command_skips_unselected_and_disables_gates() {
        let cmd = report_command(
            &ReportArgs {
                root: Path::new("/p"),
                samplesheet: "samplesheets/x.csv",
                outdir: "results/x",
                treatment: "drug",
                control: "dmso",
                moa: "MEK inhibitor",
                tcga_project: "TCGA-BRCA",
                label_check: false,
                gates: false,
                selected: [true, false, true, false],
            },
            "conda",
        );
        assert!(cmd.contains("render 01_differential_expression.qmd"));
        assert!(!cmd.contains("02_mechanism"));
        assert!(cmd.contains("'outdir:/p/results/x/tcga_brca'"));
        assert!(cmd.contains("'gate_direction:none'") && cmd.contains("'gate_pathway:none'"));
        assert!(cmd.contains("'counts:/p/results/x/featurecounts/counts.txt'"));
    }
}
