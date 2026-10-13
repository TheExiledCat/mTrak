use std::{
    env, fs,
    io::{self, BufRead, IsTerminal, Read, Write},
};

use serde_json::Value;
use ureq::{
    Agent,
    tls::{RootCerts, TlsConfig, TlsProvider},
};

use crate::data::config::COMMAND;

const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/TheExiledCat/mTrak/releases/latest";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const TARGET: &str = env!("TARGET");
const MAX_DOWNLOAD_SIZE: u64 = 64 * 1024 * 1024;

pub enum UpdateOutcome {
    Continue,
    Updated(String),
}

struct Release {
    version: String,
    download_url: String,
}

pub fn check_for_update() -> UpdateOutcome {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return UpdateOutcome::Continue;
    }
    let agent = agent();
    let Some(release) = newer_release(&agent) else {
        return UpdateOutcome::Continue;
    };
    let prompt = format!(
        "A new version of mTrak is available: v{} -> v{}. Update now? [y/n] ",
        VERSION, release.version
    );
    if !confirm(&prompt).unwrap_or(false) {
        return UpdateOutcome::Continue;
    }
    return match install(&agent, &release) {
        Ok(()) => UpdateOutcome::Updated(release.version),
        Err(e) => {
            eprintln!("Update failed: {}", e);
            UpdateOutcome::Continue
        }
    };
}

fn agent() -> Agent {
    let tls = TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    let config = Agent::config_builder()
        .tls_config(tls)
        .user_agent(format!("{}/{}", COMMAND, VERSION))
        .build();
    return Agent::new_with_config(config);
}

fn newer_release(agent: &Agent) -> Option<Release> {
    let body = agent
        .get(LATEST_RELEASE_URL)
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?
        .body_mut()
        .read_to_string()
        .ok()?;
    let json: Value = serde_json::from_str(&body).ok()?;
    let version = json["tag_name"]
        .as_str()?
        .trim_start_matches('v')
        .to_owned();
    if parse_version(&version)? <= parse_version(VERSION)? {
        return None;
    }
    let asset = json["assets"].as_array()?.iter().find(|asset| {
        asset["name"]
            .as_str()
            .is_some_and(|name| name.contains(TARGET))
    })?;
    let download_url = asset["browser_download_url"].as_str()?.to_owned();
    return Some(Release {
        version,
        download_url,
    });
}

fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.').map(|part| part.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    return parts.next().is_none().then_some(version);
}

fn install(agent: &Agent, release: &Release) -> Result<(), Box<dyn std::error::Error>> {
    let archive = download(agent, &release.download_url)?;
    let mut archive = zip::ZipArchive::new(io::Cursor::new(archive))?;
    let bin_name = format!("{}{}", COMMAND, env::consts::EXE_SUFFIX);
    let mut binary = Vec::new();
    archive.by_name(&bin_name)?.read_to_end(&mut binary)?;

    replace_exe(&binary)?;
    return Ok(());
}

#[cfg(unix)]
fn replace_exe(binary: &[u8]) -> io::Result<()> {
    let exe = env::current_exe()?.canonicalize()?;
    let staged = exe.with_file_name(format!(".{}-update", COMMAND));
    fs::write(&staged, binary)?;
    let result = fs::set_permissions(&staged, exe.metadata()?.permissions())
        .and_then(|()| fs::rename(&staged, &exe));
    if result.is_err() {
        let _ = fs::remove_file(&staged);
    }
    return result;
}

#[cfg(windows)]
fn replace_exe(binary: &[u8]) -> io::Result<()> {
    let exe = env::current_exe()?;
    let old = old_exe(&exe);
    let _ = fs::remove_file(&old);
    fs::rename(&exe, &old)?;
    if let Err(e) = fs::write(&exe, binary) {
        let _ = fs::remove_file(&exe);
        let _ = fs::rename(&old, &exe);
        return Err(e);
    }
    return Ok(());
}

#[cfg(windows)]
fn old_exe(exe: &std::path::Path) -> std::path::PathBuf {
    return exe.with_extension("old.exe");
}

pub fn remove_old_exe() {
    #[cfg(windows)]
    if let Ok(exe) = env::current_exe() {
        let _ = fs::remove_file(old_exe(&exe));
    }
}

fn download(agent: &Agent, url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut response = agent.get(url).call()?;
    let total = response.body().content_length();
    let mut reader = response
        .body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD_SIZE)
        .reader();
    let mut data = Vec::new();
    let mut chunk = [0; 16 * 1024];
    let mut stdout = io::stdout();
    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        data.extend_from_slice(&chunk[..read]);
        if let Some(total) = total.filter(|total| *total > 0) {
            print!("\rDownloading... {:3}%", data.len() as u64 * 100 / total);
            stdout.flush()?;
        }
    }
    println!();
    return Ok(data);
}

fn confirm(prompt: &str) -> io::Result<bool> {
    let mut stdout = io::stdout();
    let stdin = io::stdin();
    loop {
        print!("{}", prompt);
        stdout.flush()?;
        let mut answer = String::new();
        if stdin.lock().read_line(&mut answer)? == 0 {
            return Ok(false);
        }
        match answer.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => {}
        }
    }
}
