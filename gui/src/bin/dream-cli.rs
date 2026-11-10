//! Command-line version of the DREAM desktop app (shipped as Run_Dream-cli):
//! the same checks, commands and defaults, run in the terminal one job at a time.

#[allow(dead_code)]
#[path = "../project.rs"]
mod project;

use clap::{Args, Parser, Subcommand};
use project::{PipelineArgs, ReportArgs, State, Status};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

#[derive(Parser)]
#[command(name = "Run_Dream-cli", version, about = "Run the DREAM RNA-seq and drug repurposing workflow from a terminal.\nWith no command, shows the status.")]
struct Cli {
    /// DREAM folder (default: the folder this program is in, then the one the desktop app last used, then the current folder)
    #[arg(long, global = true)]
    dir: Option<PathBuf>,
    /// Print the commands instead of running them
    #[arg(long, global = true)]
    dry_run: bool,
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show which software, inputs and reference data are in place
    Status {
        #[arg(long, default_value = "TCGA-LUAD")]
        tcga_project: String,
    },
    /// Create the missing conda environments and download the missing data (about 19 GB in all)
    Setup {
        /// Only this part (default: everything that is missing)
        #[arg(value_parser = ["envs", "inputs", "lincs"])]
        part: Option<String>,
        /// TCGA project to download
        #[arg(long, default_value = "TCGA-LUAD")]
        tcga_project: String,
    },
    /// Run the Nextflow pipeline: reads to gene counts, with quality control
    Run {
        #[command(flatten)]
        data: Data,
        /// Library strandedness (check the featureCounts section of MultiQC if unsure)
        #[arg(long, value_parser = ["unstranded", "forward", "reverse"])]
        strandedness: String,
        /// Start from scratch instead of reusing finished steps
        #[arg(long)]
        no_resume: bool,
    },
    /// Render the Quarto reports, in order
    Report {
        /// Reports to render (default: all four)
        #[arg(value_parser = ["01", "02", "03", "04"])]
        reports: Vec<String>,
        #[command(flatten)]
        data: Data,
        #[arg(long, default_value = "trametinib")]
        treatment: String,
        #[arg(long, default_value = "control")]
        control: String,
        /// LINCS mechanism of action that report 04 expects among the top mimics
        #[arg(long, default_value = "MEK inhibitor")]
        moa: String,
        #[arg(long, default_value = "TCGA-LUAD")]
        tcga_project: String,
        /// Skip the GEO label check in report 01 (it only runs for GSE197555_A549)
        #[arg(long)]
        no_label_check: bool,
        /// Keep going when a report's built-in sanity check fails
        #[arg(long)]
        no_gates: bool,
    },
    /// List the rendered reports
    Results,
}

#[derive(Args)]
struct Data {
    #[arg(long, default_value = "samplesheets/GSE197555_A549.csv")]
    samplesheet: String,
    /// Results folder (default: results/<samplesheet name>)
    #[arg(long)]
    outdir: Option<String>,
}

impl Data {
    fn stem(&self) -> String {
        Path::new(&self.samplesheet).file_stem().unwrap_or_default().to_string_lossy().into()
    }

    fn outdir(&self) -> String {
        self.outdir.clone().unwrap_or_else(|| format!("results/{}", self.stem()))
    }
}

/// An error message and the exit code to leave with.
struct Fail(String, u8);

impl From<&str> for Fail {
    fn from(msg: &str) -> Self {
        Fail(msg.into(), 1)
    }
}

/// The name this program was started as, for hints (Run_Dream-cli in a release).
fn me() -> String {
    std::env::args().next().map(|a| a.rsplit('/').next().unwrap_or("dream-cli").to_string()).unwrap_or("dream-cli".into())
}

fn root(dir: Option<PathBuf>) -> Result<PathBuf, Fail> {
    match dir {
        Some(d) if project::is_dream(&d) => Ok(std::path::absolute(&d).unwrap_or(d)),
        Some(d) => Err(Fail(format!("{} has no pipeline/main.nf. Pass the DREAM folder with --dir.", d.display()), 1)),
        None => project::find().ok_or("No DREAM folder found. Run this inside the DREAM folder, or pass --dir.".into()),
    }
}

/// Runs one step with bash in the DREAM folder, output straight to the terminal.
/// Ctrl-C reaches the whole job (Nextflow, Java, R) because it shares the terminal's process group.
fn run(root: &Path, name: &str, script: &str, env: &[(&str, &str)], dry_run: bool) -> Result<(), Fail> {
    eprintln!("==> {name}\n$ {script}");
    if dry_run {
        return Ok(());
    }
    let status = Command::new("bash")
        .arg("-c")
        .arg(format!("set -eo pipefail\n{script}"))
        .current_dir(root)
        .envs(env.iter().copied())
        .status()
        .map_err(|e| Fail(format!("could not start bash: {e}"), 1))?;
    match status.code() {
        Some(0) => Ok(()),
        Some(code) => Err(Fail(format!("{name} failed with exit code {code}"), code as u8)),
        None => Err(Fail(format!("{name} was stopped"), 130)),
    }
}

/// A missing piece stops the step, except in a dry run where it is only reported.
fn check(blocker: Option<&str>, dry_run: bool) -> Result<(), Fail> {
    match blocker {
        Some(why) if dry_run => {
            eprintln!("note: {why}");
            Ok(())
        }
        Some(why) => Err(Fail(format!("{why}. See `{} status`.", me()), 1)),
        None => Ok(()),
    }
}

fn print_status(root: &Path, st: &Status) {
    println!("DREAM folder  {}", project::tilde(root));
    let groups: [(&str, Vec<&project::Check>); 3] = [
        ("Setup step 1 · Software", st.envs.iter().collect()),
        ("Setup step 2 · Reference and inputs", st.inputs.iter().collect()),
        ("Setup step 3 · LINCS 2020", vec![&st.lincs]),
    ];
    for (title, checks) in groups {
        println!("\n{title}");
        for c in checks {
            let mark = match c.state {
                State::Ok => "✓",
                State::Partial => "~",
                State::Missing => "✗",
            };
            println!("  {mark} {:<24} {}", c.label, c.detail);
        }
    }
    if let Some((free, total)) = project::memory() {
        println!("\nMemory  {} free of {}", project::size_text(free), project::size_text(total));
    }
    let all = st.envs.iter().chain(&st.inputs).chain([&st.lincs]);
    if all.clone().any(|c| c.state != State::Ok) {
        println!("\nMissing pieces: `{} setup` creates the environments and downloads the data.", me());
    }
}

fn setup(root: &Path, st: &Status, part: Option<&str>, tcga_project: &str, dry_run: bool) -> Result<(), Fail> {
    let want = |p: &str| part.is_none_or(|x| x == p);
    let mut steps: Vec<(String, String, Vec<(&str, &str)>)> = Vec::new();
    let mut qc_ready = st.ok("dream-qc environment");
    if (want("envs") || want("inputs")) && st.conda.is_none() {
        return Err("Conda not found. Install Miniforge first: https://github.com/conda-forge/miniforge".into());
    }
    if want("envs") {
        for (label, file, name) in [("dream-qc environment", "envs/qc.yml", "dream-qc"), ("dream-r environment", "envs/r.yml", "dream-r")] {
            if !st.ok(label) {
                steps.push((format!("Create {name}"), format!("{} env create -y -f {file}", st.conda_bin("mamba")), vec![]));
                qc_ready |= name == "dream-qc";
            }
        }
        for (label, url) in [("Nextflow", "https://www.nextflow.io/docs/latest/install.html"), ("Java", "https://adoptium.net/")] {
            if !st.ok(label) {
                eprintln!("note: {label} is not on PATH and must be installed by hand: {url}");
            }
        }
    }
    if want("inputs") && st.inputs.iter().any(|c| c.state != State::Ok) {
        if !qc_ready {
            return Err(Fail(format!("Downloading inputs needs the dream-qc environment: run `{} setup envs` first", me()), 1));
        }
        let script = format!("{} run -n dream-qc --no-capture-output bash scripts/get_inputs.sh", st.conda_bin("conda"));
        steps.push(("Download inputs (about 12 GB)".into(), script, vec![("TCGA_PROJECT", tcga_project)]));
    }
    if want("lincs") && st.lincs.state != State::Ok {
        steps.push(("Download LINCS 2020 (6.3 GB)".into(), "bash scripts/download_lincs.sh".into(), vec![]));
    }
    if steps.is_empty() {
        println!("Nothing to set up.");
    }
    for (name, script, env) in steps {
        run(root, &name, &script, &env, dry_run)?;
    }
    Ok(())
}

fn selection(reports: &[String]) -> [bool; 4] {
    let mut picked = [reports.is_empty(); 4];
    for r in reports {
        picked[r.parse::<usize>().unwrap() - 1] = true;
    }
    picked
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli_main(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Fail(msg, code)) => {
            eprintln!("error: {msg}");
            ExitCode::from(code)
        }
    }
}

fn cli_main(cli: Cli) -> Result<(), Fail> {
    let command = cli.command.unwrap_or(Cmd::Status { tcga_project: "TCGA-LUAD".into() });
    let root = root(cli.dir)?;
    let dry = cli.dry_run;
    match command {
        Cmd::Status { tcga_project } => print_status(&root, &Status::read(&root, &tcga_project)),
        Cmd::Setup { part, tcga_project } => setup(&root, &Status::read(&root, &tcga_project), part.as_deref(), &tcga_project, dry)?,
        Cmd::Run { data, strandedness, no_resume } => {
            let st = Status::read(&root, "TCGA-LUAD");
            check(project::pipeline_blocker(&st, &root, &data.samplesheet), dry)?;
            let outdir = data.outdir();
            let args = PipelineArgs { samplesheet: &data.samplesheet, strandedness: &strandedness, outdir: &outdir, resume: !no_resume };
            run(&root, "Pipeline", &project::pipeline_command(&args), &[], dry)?;
        }
        Cmd::Report { reports, data, treatment, control, moa, tcga_project, no_label_check, no_gates } => {
            let st = Status::read(&root, &tcga_project);
            let outdir = data.outdir();
            let args = ReportArgs {
                root: &root,
                samplesheet: &data.samplesheet,
                outdir: &outdir,
                treatment: &treatment,
                control: &control,
                moa: &moa,
                tcga_project: &tcga_project,
                label_check: !no_label_check && data.stem() == "GSE197555_A549",
                gates: !no_gates,
                selected: selection(&reports),
            };
            check(project::report_blocker(&st, &args), dry)?;
            run(&root, "Reports", &project::report_command(&args, &st.conda_bin("conda")), &[], dry)?;
        }
        Cmd::Results => {
            for group in project::reports(&root) {
                println!("{}", group.name);
                if group.reports.is_empty() {
                    println!("  none yet");
                }
                for r in group.reports {
                    let age = r.modified.map(project::ago).unwrap_or_default();
                    println!("  {:<54} {:<12} {}", r.title, age, project::tilde(&r.path));
                }
                println!();
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn options_are_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn report_defaults_follow_the_samplesheet() {
        let cli = Cli::try_parse_from(["x", "report", "02", "04", "--samplesheet", "samplesheets/other.csv"]).ok().unwrap();
        let Some(Cmd::Report { reports, data, .. }) = cli.command else { panic!("not a report command") };
        assert_eq!(selection(&reports), [false, true, false, true]);
        assert_eq!(selection(&[]), [true; 4]);
        assert_eq!(data.outdir(), "results/other");
        assert_ne!(data.stem(), "GSE197555_A549");
    }

    #[test]
    fn run_requires_strandedness() {
        assert!(Cli::try_parse_from(["x", "run"]).is_err());
        assert!(Cli::try_parse_from(["x", "run", "--strandedness", "sideways"]).is_err());
    }
}
