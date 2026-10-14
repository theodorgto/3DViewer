use crate::model::{self, Model};
use anyhow::{bail, Context, Result};
use bincode::Options;
use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const IMPORT_TIMEOUT: Duration = Duration::from_secs(180);

pub struct ImportJob {
    child: Child,
    directory: tempfile::TempDir,
    pub path: PathBuf,
    started: Instant,
    deadline: Duration,
}

impl ImportJob {
    pub fn start(path: PathBuf, tolerance: f64) -> Result<Self> {
        let directory = tempfile::Builder::new()
            .prefix("model-viewer-import-")
            .tempdir()?;
        let log = File::create(directory.path().join("import.log"))?;
        let mut command = Command::new(std::env::current_exe()?);
        command
            .args(["--import-worker"])
            .arg(&path)
            .arg(tolerance.to_string())
            .arg(directory.path().join("model.bin"))
            .env("MODEL_VIEWER_IMPORT_TRACE", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(log));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let child = command
            .spawn()
            .context("Cannot start isolated model importer")?;
        Ok(Self {
            child,
            directory,
            path,
            started: Instant::now(),
            deadline: IMPORT_TIMEOUT,
        })
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    pub fn progress(&self) -> String {
        fs::read_to_string(self.directory.path().join("import.log"))
            .unwrap_or_default()
            .lines()
            .rev()
            .find(|line| line.starts_with("STEP:"))
            .unwrap_or("Reading model")
            .to_owned()
    }

    pub fn poll(&mut self) -> Result<Option<Model>> {
        if let Some(status) = self.child.try_wait()? {
            if !status.success() {
                let log = fs::read_to_string(self.directory.path().join("import.log"))
                    .unwrap_or_default();
                let detail = log
                    .lines()
                    .rev()
                    .find(|line| line.starts_with("Import failed:"))
                    .unwrap_or("The CAD importer stopped unexpectedly");
                bail!("{detail}");
            }
            let file = File::open(self.directory.path().join("model.bin"))
                .context("Importer produced no model")?;
            let model = bincode::DefaultOptions::new()
                .with_limit(1_073_741_824)
                .deserialize_from(BufReader::new(file))
                .context("Cannot read imported model")?;
            return Ok(Some(model));
        }
        if self.elapsed() >= self.deadline {
            self.child
                .kill()
                .context("Cannot stop timed-out importer")?;
            self.child.wait()?;
            bail!("Import timed out after {} seconds. Try a coarser STEP tolerance or export a mesh from your CAD application.", self.deadline.as_secs());
        }
        Ok(None)
    }
}

impl Drop for ImportJob {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn run_worker(path: &Path, tolerance: f64, output: &Path) -> Result<()> {
    let model = load_worker_model(path, tolerance, output)?;
    let mut writer = BufWriter::new(File::create(output)?);
    bincode::DefaultOptions::new().serialize_into(&mut writer, &model)?;
    writer.flush()?;
    Ok(())
}

pub fn export_worker(path: &Path, tolerance: f64, output: &Path) -> Result<()> {
    let model = load_worker_model(path, tolerance, output)?;
    let mut writer = BufWriter::new(File::create(output)?);
    serde_json::to_writer(&mut writer, &model)?;
    writer.flush()?;
    Ok(())
}

fn load_worker_model(path: &Path, tolerance: f64, output: &Path) -> Result<Model> {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let staged;
    let path = if matches!(extension.as_str(), "step" | "stp") {
        staged = output.with_file_name("input.step");
        fs::copy(path, &staged).context("Cannot read STEP file")?;
        staged.as_path()
    } else {
        path
    };
    let mut model = model::load(path, tolerance)?;
    model.name = name;
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webview_export_preserves_model_data() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("model.json");
        export_worker(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/pyramid.ply"),
            0.001,
            &output,
        )
        .unwrap();
        let decoded: Model = serde_json::from_reader(File::open(output).unwrap()).unwrap();
        assert_eq!(decoded.name, "pyramid.ply");
        assert_eq!(decoded.vertices.len(), 5);
        assert_eq!(decoded.indices.len(), 18);
        assert!(decoded.warnings.is_empty());
        assert!((decoded.dimensions.z - 1.8).abs() < 1e-6);
    }

    #[test]
    fn worker_transfer_preserves_geometry_and_warnings() {
        let mut model = model::demo();
        model.warnings.push("Incomplete STEP model".into());
        let bytes = bincode::DefaultOptions::new().serialize(&model).unwrap();
        let decoded: Model = bincode::DefaultOptions::new().deserialize(&bytes).unwrap();
        assert_eq!(decoded.indices, model.indices);
        assert_eq!(decoded.warnings, model.warnings);
        assert_eq!(decoded.dimensions, model.dimensions);
    }

    #[cfg(windows)]
    fn waiting_job() -> ImportJob {
        let directory = tempfile::tempdir().unwrap();
        let child = Command::new("cmd")
            .args(["/D", "/C", "pause"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        ImportJob {
            child,
            directory,
            path: PathBuf::new(),
            started: Instant::now(),
            deadline: IMPORT_TIMEOUT,
        }
    }

    #[test]
    #[cfg(windows)]
    fn timeout_stops_importer() {
        let mut job = waiting_job();
        job.deadline = Duration::ZERO;
        assert!(job.poll().err().unwrap().to_string().contains("timed out"));
        assert!(job.child.try_wait().unwrap().is_some());
    }

    #[test]
    #[cfg(windows)]
    fn cancelling_stops_importer() {
        let job = waiting_job();
        let identifier = job.child.id();
        drop(job);
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {identifier}"), "/FO", "CSV", "/NH"])
            .output()
            .unwrap();
        assert!(!String::from_utf8_lossy(&output.stdout).contains("cmd.exe"));
    }
}
