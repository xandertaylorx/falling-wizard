//! Self-update: compare the commit this build came from with the latest on GitHub, and
//! re-run the install script in the background when asked.

use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

pub const REPO: &str = "https://github.com/xandertaylorx/falling-wizard.git";
pub const INSTALL: &str = "https://raw.githubusercontent.com/xandertaylorx/falling-wizard/main/web/install.sh";

#[derive(Clone, PartialEq, Debug)]
pub enum Status {
    Checking,
    UpToDate,
    Available,
    Updating,
    Ready,
    Failed,
    /// Built outside git (no version to compare).
    Unknown,
}

pub struct Updater {
    status: Arc<Mutex<Status>>,
    /// While updating: what it's doing and how far along (0..1).
    progress: Arc<Mutex<(String, f32)>>,
}

impl Updater {
    /// Start a background check against GitHub.
    pub fn start() -> Self {
        let status = Arc::new(Mutex::new(Status::Checking));
        let s = status.clone();
        std::thread::spawn(move || {
            let mine = env!("FW_COMMIT");
            let result = if mine.is_empty() {
                Status::Unknown
            } else {
                match Command::new("git").args(["ls-remote", REPO, "refs/heads/main"]).stderr(Stdio::null()).output() {
                    Ok(o) if o.status.success() => {
                        let latest = String::from_utf8_lossy(&o.stdout).split_whitespace().next().unwrap_or("").to_string();
                        if latest.is_empty() || latest == mine {
                            Status::UpToDate
                        } else {
                            Status::Available
                        }
                    }
                    _ => Status::Unknown,
                }
            };
            *s.lock().unwrap() = result;
        });
        Self { status, progress: Arc::new(Mutex::new((String::new(), 0.0))) }
    }

    pub fn progress(&self) -> (String, f32) {
        self.progress.lock().unwrap().clone()
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    /// Run the installer in the background (same command as the website),
    /// following its output to report progress.
    pub fn update(&self) {
        use std::io::{BufRead, BufReader};
        *self.status.lock().unwrap() = Status::Updating;
        *self.progress.lock().unwrap() = ("STARTING".into(), 0.02);
        let (s, prog) = (self.status.clone(), self.progress.clone());
        std::thread::spawn(move || {
            let set = |stage: &str, p: f32| *prog.lock().unwrap() = (stage.to_string(), p);
            let child = Command::new("bash")
                .arg("-c")
                .arg(format!("curl -fsSL {INSTALL} | FW_NO_OPEN=1 FW_PROGRESS=1 bash 2>&1"))
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();
            let Ok(mut child) = child else {
                *s.lock().unwrap() = Status::Failed;
                return;
            };
            let (mut total, mut done) = (60.0f32, 0.0f32);
            if let Some(out) = child.stdout.take() {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    if let Some(n) = line.strip_prefix("::total:") {
                        total = n.trim().parse().unwrap_or(60.0f32).max(1.0);
                    } else if line.contains("Fetching") || line.contains("Updating") {
                        set("DOWNLOADING", 0.06);
                    } else if line.contains("Installing Rust") {
                        set("INSTALLING RUST", 0.04);
                    } else if line.contains("Building") {
                        set("BUILDING", 0.1);
                    } else if line.trim_start().starts_with("Compiling") {
                        done += 1.0;
                        set("BUILDING", 0.1 + (done / total).min(1.0) * 0.82);
                    } else if line.trim_start().starts_with("Finished") {
                        set("BUILDING", 0.93);
                    } else if line.contains("Installing") {
                        set("INSTALLING", 0.97);
                    }
                }
            }
            let ok = child.wait().map(|st| st.success()).unwrap_or(false);
            set(if ok { "DONE" } else { "FAILED" }, 1.0);
            *s.lock().unwrap() = if ok { Status::Ready } else { Status::Failed };
        });
    }

    /// Reopen the freshly installed game and quit this one.
    pub fn relaunch() -> ! {
        let app = std::env::var("HOME").map(|h| format!("{h}/Applications/Falling Wizard.app")).unwrap_or_default();
        if std::path::Path::new(&app).exists() {
            let _ = Command::new("open").args(["-n", &app]).spawn();
        } else if let Ok(exe) = std::env::current_exe() {
            let _ = Command::new(exe).spawn();
        }
        std::process::exit(0);
    }
}

/// `falling-wizard --update`: update from the terminal.
pub fn update_now() {
    println!("Updating Falling Wizard...");
    let ok = Command::new("bash").arg("-c").arg(format!("curl -fsSL {INSTALL} | FW_NO_OPEN=1 bash")).status().map(|s| s.success()).unwrap_or(false);
    if !ok {
        eprintln!("Update failed. Check your internet connection and try again.");
        std::process::exit(1);
    }
}
