//! The game data: the first run's choice of a folder or the original ZIP (M13-02) and the host
//! side of the options' game-data page (M11-03).
//!
//! - [`first_run`] asks with the system's message boxes and file dialogs (the game's fonts are
//!   not available yet) until the player chooses usable data or quits. Unusable data is
//!   described in a message box; usable data is then shown on the game-data page in the game's
//!   own fonts ([`td_flow::front::OptionsScreen::data_setup`]), where a ZIP can be extracted.
//! - [`DataHost`] owns the data in use, answers the page's requests (dialogs, validation,
//!   extraction) and switches to a new choice when the start screen asks
//!   (`td_flow::front::data`).
//!
//! A ZIP is read in place (`td_assets::archive`); extracting it, which the player chooses on the
//! page, copies only the manifest files into `<user data>/game` (`td_assets::import`).

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use td_assets::{DataDir, DataDirKind, Episode, ExeResources, ValidationReport, validate};
use td_flow::front::{DataInfo, DataRequest, DataSource, DataSummary};
use td_platform::Platform;

const TITLE: &str = "TD2192-Modern";

/// Asks for the game data until the player chooses usable data (Episode 1 complete) or quits.
/// Returns the folder or ZIP chosen.
pub fn first_run() -> Result<PathBuf> {
    let mut question = "Welcome! TD2192-Modern plays your own copy of Traffic Department 2192.\n\n\
        Where is it?\n\
        - Folder: an installed copy (the folder with TD1.EXE and FILE0001.EXE), used where it is.\n\
        - ZIP file: the original distribution archive, read where it is. You can extract it \
        into your user folder on the next screen."
        .to_string();
    loop {
        let path = match td_platform::ask(TITLE, &question, &["Folder...", "ZIP file...", "Quit"]) {
            Some(0) => td_platform::pick_folder()?,
            Some(1) => td_platform::pick_zip()?,
            _ => bail!("No game data was chosen. Start again, or run with --data <folder or zip>."),
        };
        let Some(path) = path else {
            continue;
        };
        match DataDir::open(&path) {
            Ok(dir) => {
                let report = validate(&dir);
                if report.playable(Episode::E1) {
                    return Ok(path);
                }
                question = format!(
                    "{}\n\nPlease choose another folder or ZIP file.",
                    unusable_text(&dir, &report)
                );
            }
            Err(e) => {
                question = format!("{e}\n\nPlease choose a folder or the original ZIP file.");
            }
        }
    }
}

/// Why `dir` cannot be used, for a message box.
fn unusable_text(dir: &DataDir, report: &ValidationReport) -> String {
    let s = DataSummary::of(dir, report);
    let what = match dir.kind() {
        DataDirKind::Folder => "folder",
        DataDirKind::Archive => "ZIP file",
    };
    let mut missing: Vec<&str> = report
        .checks
        .iter()
        .filter(|c| c.entry.required && c.entry.used_by.contains(Episode::E1.usage()))
        .filter(|c| !c.status.is_present())
        .map(|c| c.entry.name)
        .collect();
    let more = missing.len().saturating_sub(8);
    missing.truncate(8);
    let mut text = format!(
        "{} does not hold a complete Traffic Department 2192 (Episode 1 is needed).\n\n\
         Files: {}.\nPlayable: {}.",
        dir.root().display(),
        s.files_line(),
        s.playable_line()
    );
    if !missing.is_empty() {
        text.push_str(&format!(
            "\nMissing from this {what}: {}",
            missing.join(", ")
        ));
        if more > 0 {
            text.push_str(&format!(" and {more} more"));
        }
        text.push('.');
    }
    text
}

/// The game data in use and the game-data page's requests.
#[derive(Debug)]
pub struct DataHost {
    dir: DataDir,
    resources: ExeResources,
    /// A usable new choice, used from the start screen on.
    pending: Option<DataDir>,
    info: DataInfo,
}

impl DataHost {
    /// The host for `dir` (validated as `report`, its text in `resources`), named by `source`;
    /// ZIPs are extracted into `import_dir`.
    pub fn new(
        dir: DataDir,
        resources: ExeResources,
        report: &ValidationReport,
        source: DataSource,
        import_dir: PathBuf,
    ) -> Self {
        let info = DataInfo {
            current: DataSummary::of(&dir, report),
            source,
            pending: None,
            message: None,
            import_dir,
        };
        Self {
            dir,
            resources,
            pending: None,
            info,
        }
    }

    /// The data in use.
    pub fn data(&self) -> &DataDir {
        &self.dir
    }

    /// The exe resources of the data in use.
    pub fn resources(&self) -> &ExeResources {
        &self.resources
    }

    /// What the game-data page shows.
    pub fn info(&self) -> &DataInfo {
        &self.info
    }

    /// Answers `request`: shows the dialog or extracts, then validates. Returns the folder or
    /// ZIP to save as `data_dir` when a usable new choice was made.
    pub fn handle(&mut self, request: DataRequest, platform: &mut Platform) -> Option<PathBuf> {
        self.answer(request, |r| {
            match r {
                DataRequest::PickFolder => platform.pick_folder(),
                _ => platform.pick_zip(),
            }
            .map_err(|e| e.to_string())
        })
    }

    /// [`Self::handle`] with `pick` for the dialogs. Nothing happens while the command line or
    /// the environment names the data (MDG §8.8).
    fn answer(
        &mut self,
        request: DataRequest,
        pick: impl FnOnce(DataRequest) -> Result<Option<PathBuf>, String>,
    ) -> Option<PathBuf> {
        if !self.info.changeable() {
            return None;
        }
        if request == DataRequest::Extract {
            return self.extract();
        }
        match pick(request) {
            Ok(Some(path)) => self.choose(&path),
            Ok(None) => None,
            Err(e) => {
                self.info.message = Some(format!("the dialog failed - {e}"));
                None
            }
        }
    }

    /// Validates `path` as the new choice. Usable data becomes pending (and is returned, to be
    /// saved); anything else leaves the data as it was, with a message saying why.
    pub fn choose(&mut self, path: &Path) -> Option<PathBuf> {
        if same_path(path, self.dir.root()) {
            self.pending = None;
            self.info.pending = None;
            self.info.message = Some("that is the game data in use".into());
            return Some(path.to_path_buf());
        }
        let dir = match DataDir::open(path) {
            Ok(dir) => dir,
            Err(e) => {
                self.info.message = Some(format!("nothing changed - {e}"));
                return None;
            }
        };
        let report = validate(&dir);
        let summary = DataSummary::of(&dir, &report);
        if !summary.usable() {
            self.info.message = Some(format!(
                "nothing changed - no complete episode 1 - {}",
                summary.files_line()
            ));
            return None;
        }
        self.info.pending = Some(summary);
        self.info.message = Some("saved to settings.toml".into());
        self.pending = Some(dir);
        Some(path.to_path_buf())
    }

    /// Extracts the ZIP shown (the new choice, else the data in use) into the import folder
    /// and chooses the copy.
    fn extract(&mut self) -> Option<PathBuf> {
        let src = match &self.pending {
            Some(p) => p,
            None => &self.dir,
        };
        if src.kind() != DataDirKind::Archive {
            return None;
        }
        let dest = self.info.import_dir.clone();
        match td_assets::extract(src, &dest) {
            Ok(summary) => {
                let saved = self.choose(&dest);
                if saved.is_some() {
                    self.info.message = Some(format!(
                        "extracted {} files - the zip is no longer needed",
                        summary.written
                    ));
                }
                saved
            }
            Err(e) => {
                self.info.message = Some(format!("could not extract - {e}"));
                None
            }
        }
    }

    /// Switches to the pending choice (the start screen is on top). False if there is none.
    pub fn switch(&mut self) -> bool {
        let Some(dir) = self.pending.take() else {
            return false;
        };
        match ExeResources::load(&dir) {
            Ok(resources) => {
                tracing::info!("game data: now {}", dir.root().display());
                self.dir = dir;
                self.resources = resources;
                if let Some(p) = self.info.pending.take() {
                    self.info.current = p;
                }
                self.info.source = DataSource::Chosen;
                self.info.message = None;
                true
            }
            Err(e) => {
                self.info.pending = None;
                self.info.message = Some(format!("could not switch - {e}"));
                false
            }
        }
    }
}

/// True if `a` and `b` name the same file or folder.
fn same_path(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("td2192-data-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn host_for(dir: &Path, source: DataSource, import: PathBuf) -> DataHost {
        let data = DataDir::open(dir).unwrap();
        let report = validate(&data);
        DataHost::new(data, ExeResources::default(), &report, source, import)
    }

    #[test]
    fn an_unusable_choice_changes_nothing() {
        let dir = temp("unusable");
        let empty = dir.join("empty");
        fs::create_dir_all(&empty).unwrap();
        let mut host = host_for(&dir, DataSource::Settings, dir.join("game"));
        assert_eq!(host.choose(&empty), None);
        assert!(host.info().pending.is_none());
        assert!(
            host.info()
                .message
                .as_deref()
                .unwrap()
                .contains("nothing changed")
        );
        assert!(!host.switch());
        let junk = dir.join("junk.zip");
        fs::write(&junk, b"no zip").unwrap();
        assert_eq!(host.choose(&junk), None);
        assert!(!host.switch());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_data_in_use_is_not_chosen_again() {
        let dir = temp("same");
        let mut host = host_for(&dir, DataSource::Settings, dir.join("game"));
        assert_eq!(host.choose(&dir), Some(dir.clone()));
        assert!(host.info().pending.is_none());
        assert!(!host.switch());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unusable_text_names_the_missing_files() {
        let dir = temp("text");
        let data = DataDir::open(&dir).unwrap();
        let text = unusable_text(&data, &validate(&data));
        assert!(text.contains("Episode 1 is needed"), "{text}");
        assert!(text.contains("Missing from this folder: "), "{text}");
        assert!(text.contains("more."), "{text}");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The original files zipped at test time (never committed): the ZIP is chosen, read in
    /// place, extracted, and the start screen's switch uses the copy.
    #[test]
    fn a_zip_is_chosen_extracted_and_switched_to() {
        let data = td_test_support::require_data!();
        let install = DataDir::open(&data).unwrap();
        let dir = temp("zip");
        let zip_path = dir.join("TD2192.ZIP");
        {
            use std::io::Write;
            let mut w = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for name in install.names() {
                w.start_file(name.to_uppercase(), opts).unwrap();
                w.write_all(&install.read(name).unwrap()).unwrap();
            }
            w.finish().unwrap();
        }
        let elsewhere = dir.join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        let import = dir.join("game");
        let mut host = host_for(&elsewhere, DataSource::Settings, import.clone());
        assert_eq!(host.choose(&zip_path), Some(zip_path.clone()));
        let pending = host.info().pending.clone().unwrap();
        assert!(pending.archive && pending.usable());
        assert_eq!(pending.playable, [true; 3]);
        assert_eq!(host.extract(), Some(import.clone()));
        let pending = host.info().pending.clone().unwrap();
        assert!(!pending.archive);
        assert_eq!(pending.counts.ok, 218);
        assert!(
            host.info()
                .message
                .as_deref()
                .unwrap()
                .starts_with("extracted 218")
        );
        assert!(host.switch());
        assert_eq!(host.data().root(), import);
        assert_eq!(host.info().current, pending);
        assert_eq!(host.info().source, DataSource::Chosen);
        assert!(!host.resources().status.is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_override_locks_the_page() {
        let dir = temp("locked");
        let mut host = host_for(&dir, DataSource::Environment, dir.join("game"));
        assert!(!host.info().changeable());
        for request in [
            DataRequest::PickFolder,
            DataRequest::PickZip,
            DataRequest::Extract,
        ] {
            assert_eq!(
                host.answer(request, |_| panic!("no dialog while overridden")),
                None
            );
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_cancelled_or_failed_dialog_changes_nothing() {
        let dir = temp("dialog");
        let mut host = host_for(&dir, DataSource::Settings, dir.join("game"));
        assert_eq!(host.answer(DataRequest::PickZip, |_| Ok(None)), None);
        assert_eq!(host.info().message, None, "a cancel says nothing");
        assert_eq!(
            host.answer(DataRequest::PickFolder, |r| {
                assert_eq!(r, DataRequest::PickFolder);
                Err("no display".into())
            }),
            None
        );
        assert_eq!(
            host.info().message.as_deref(),
            Some("the dialog failed - no display")
        );
        // Extracting needs a ZIP.
        assert_eq!(
            host.answer(DataRequest::Extract, |_| panic!("no dialog")),
            None
        );
        assert!(!host.switch());
        fs::remove_dir_all(&dir).unwrap();
    }
}
