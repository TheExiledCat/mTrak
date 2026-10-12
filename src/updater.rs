use std::io::{self, BufRead, IsTerminal, Write};

use self_update::{backends::github::Update, cargo_crate_version, update::ReleaseUpdate};

use crate::data::config::COMMAND;

const REPO_OWNER: &str = "TheExiledCat";
const REPO_NAME: &str = "mTrak";
const VERSION: &str = cargo_crate_version!();

pub enum UpdateOutcome {
    Continue,
    Updated(String),
}

pub fn check_for_update() -> UpdateOutcome {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return UpdateOutcome::Continue;
    }
    return match try_update() {
        Ok(outcome) => outcome,
        Err(e) => {
            eprintln!("Update failed: {}", e);
            UpdateOutcome::Continue
        }
    };
}

fn try_update() -> Result<UpdateOutcome, Box<dyn std::error::Error>> {
    let Ok(latest) = updater(None)?.get_latest_release() else {
        return Ok(UpdateOutcome::Continue);
    };
    if !self_update::version::bump_is_greater(VERSION, &latest.version)? {
        return Ok(UpdateOutcome::Continue);
    }
    if latest
        .asset_for(self_update::get_target(), None)
        .is_none()
    {
        return Ok(UpdateOutcome::Continue);
    }
    if !confirm(&format!(
        "A new version of mTrak is available: v{} -> v{}. Update now? [y/n] ",
        VERSION, latest.version
    ))? {
        return Ok(UpdateOutcome::Continue);
    }
    updater(Some(&format!("v{}", latest.version)))?.update()?;
    return Ok(UpdateOutcome::Updated(latest.version));
}

fn updater(tag: Option<&str>) -> Result<Box<dyn ReleaseUpdate>, self_update::errors::Error> {
    let mut builder = Update::configure();
    builder
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(COMMAND)
        .current_version(VERSION)
        .show_output(false)
        .show_download_progress(true)
        .no_confirm(true);
    if let Some(tag) = tag {
        builder.target_version_tag(tag);
    }
    return builder.build();
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
