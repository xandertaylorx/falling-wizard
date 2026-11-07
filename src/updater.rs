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
        Self { status }
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    /// Run the installer in the background (same command as the website).
    pub fn update(&self) {
        *self.status.lock().unwrap() = Status::Updating;
        let s = self.status.clone();
        std::thread::spawn(move || {
            let ok = Command::new("bash")
                .arg("-c")
                .arg(format!("curl -fsSL {INSTALL} | FW_NO_OPEN=1 bash"))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|st| st.success())
                .unwrap_or(false);
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
