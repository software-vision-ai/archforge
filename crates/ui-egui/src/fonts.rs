//! Installed fonts: a script the bundled faces cannot draw is drawn in an installed family.
//!
//! The host scans and reads font files through [`FontSource`], so the shell stays portable; the
//! tests give none, and only the bundled faces draw. One worker thread scans, then reads the faces
//! the frame asks for. The frame checks each with the parser egui itself uses, so egui's font
//! loading cannot panic on it.
//!
//! egui's bundled faces have no Japanese or Chinese glyphs, so once the scan reports, an installed
//! family that draws them is loaded and put behind every family as a fallback: a Chinese one in
//! every language, so a name in those scripts draws, and a Japanese one before it while the
//! interface is in Japanese, so its kanji take the Japanese forms. Each is given to egui as
//! `fallback:<family>`, moved onto the bundled face's baseline, behind the other families. A host
//! without one (the web, or a desktop with no such font) draws those characters as egui's
//! replacement box; the log says why, and starting in Japanese or Chinese logs a notice naming
//! the language (`Unserved`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, mpsc};

use egui::FontFamily;

/// One face of an installed font file, as the host's scan reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontEntry {
    /// The family names the font reports, the typographic one first.
    pub families: Vec<String>,
    /// The PostScript name; empty when the font has none.
    pub post_script: String,
    /// The weight, 400 for a regular face.
    pub weight: u16,
    pub italic: bool,
    /// The width class, 1 to 9, 5 for a normal face.
    pub stretch: u16,
    /// Where the host finds the file again; the shell only hands it back.
    pub key: String,
    /// The face's index in its file (a collection holds several).
    pub index: u32,
    /// The file's length in bytes.
    pub len: u64,
}

/// The host's installed fonts: a scan of what is installed and a read of one face's file. Both
/// run on the worker thread, never on the frame.
pub trait FontSource: Send + Sync {
    /// Every face installed, in any order, none from a file longer than `cap`.
    fn scan(&self, cap: u64) -> Vec<FontEntry>;
    /// The bytes of the file holding `entry`, refused when longer than `cap`.
    fn read(&self, entry: &FontEntry, cap: u64) -> Result<Vec<u8>, String>;
}

/// The limits on loaded faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Caps {
    /// The largest face file read.
    pub face: u64,
    /// The bytes of every face loaded together; loaded faces stay for the process.
    pub total: u64,
    /// The faces loaded; a face that failed does not count.
    pub count: usize,
}

impl Default for Caps {
    fn default() -> Self {
        Self { face: 64 << 20, total: 256 << 20, count: 32 }
    }
}

enum Job {
    Load(String, FontEntry),
}

enum Done {
    Scanned(Table),
    Loaded(String, u32, Result<Vec<u8>, String>),
}

/// Installed families that draw Japanese, most wanted first: the platforms' interface faces
/// (macOS, Windows), then the free families Linux distributions ship.
pub(crate) const JAPANESE_FALLBACKS: &[&str] = &[
    "Hiragino Sans",
    "Hiragino Kaku Gothic ProN",
    "Yu Gothic UI",
    "Yu Gothic",
    "Meiryo UI",
    "Meiryo",
    "Noto Sans CJK JP",
    "Noto Sans JP",
    "Source Han Sans JP",
    "BIZ UDPGothic",
    "IPAexGothic",
    "IPAPGothic",
    "TakaoPGothic",
    "VL PGothic",
];

/// Installed families that draw Simplified Chinese (and kana, and the traditional forms of a
/// Japanese name), most wanted first.
pub(crate) const CHINESE_FALLBACKS: &[&str] = &[
    "PingFang SC",
    "Hiragino Sans GB",
    "Microsoft YaHei UI",
    "Microsoft YaHei",
    "Noto Sans CJK SC",
    "Noto Sans SC",
    "Source Han Sans SC",
    "Source Han Sans CN",
    "WenQuanYi Micro Hei",
    "WenQuanYi Zen Hei",
    "Droid Sans Fallback",
    "Heiti SC",
    "SimHei",
];

/// The fallback lists a language wants, in the order their families go behind the others.
pub(crate) fn fallback_lists(language: &str) -> &'static [&'static [&'static str]] {
    if language == "ja" { &[JAPANESE_FALLBACKS, CHINESE_FALLBACKS] } else { &[CHINESE_FALLBACKS] }
}

/// A scan as the shell looks faces up in it, built on the worker so the frame does no sorting.
#[derive(Default)]
pub(crate) struct Table {
    /// Every name a fallback list may give a family by, trimmed and lower-cased, to its canonical
    /// family.
    names: BTreeMap<String, String>,
    /// Each canonical family's regular face.
    regular: BTreeMap<String, FontEntry>,
}

/// The longest family name taken from an installed font.
const MAX_FAMILY: usize = 256;

/// The table of a scan: each family's regular face and the names it goes by. Families named past
/// the length limit, hidden ('.' first) or with a control character are left out. A family's own
/// name always names it; another name a font reports, then a PostScript name, only names a family
/// when no family is called that.
pub(crate) fn table(mut entries: Vec<FontEntry>) -> Table {
    // A stable order, so the same fonts always give the same table.
    entries.sort_by(|a, b| (&a.families, &a.key, a.index).cmp(&(&b.families, &b.key, b.index)));
    let mut regular: BTreeMap<String, (String, FontEntry)> = BTreeMap::new();
    for e in &entries {
        let Some(family) = e.families.first().map(|f| f.trim()) else { continue };
        if family.is_empty() || family.len() > MAX_FAMILY || family.starts_with('.') || family.chars().any(char::is_control) {
            continue;
        }
        let slot = regular.entry(family.to_lowercase()).or_insert_with(|| (family.to_string(), e.clone()));
        if irregularity(e) < irregularity(&slot.1) {
            slot.1 = e.clone();
        }
    }
    let mut names: BTreeMap<String, String> = regular.iter().map(|(key, (name, _))| (key.clone(), name.clone())).collect();
    let canonical = |e: &FontEntry| e.families.first().and_then(|f| regular.get(&f.trim().to_lowercase())).map(|r| r.0.clone());
    for other_names in [false, true] {
        for e in &entries {
            let Some(family) = canonical(e) else { continue };
            let others: Vec<&String> = if other_names { vec![&e.post_script] } else { e.families.iter().skip(1).collect() };
            for name in others {
                let key = name.trim().to_lowercase();
                if !key.is_empty() {
                    names.entry(key).or_insert_with(|| family.clone());
                }
            }
        }
    }
    Table { names, regular: regular.into_values().collect() }
}

/// The families asked of the worker: those still in flight and those that failed.
#[derive(Default)]
struct Asked {
    in_flight: BTreeSet<String>,
    failed: BTreeSet<String>,
}

/// The installed fonts as the shell knows them.
#[derive(Default)]
pub(crate) struct FontState {
    pub(crate) caps: Caps,
    started: bool,
    jobs: Option<mpsc::Sender<Job>>,
    done: Option<mpsc::Receiver<Done>>,
    scanned: bool,
    /// The scan's table; empty until the scan reports.
    table: Table,
    asked: Mutex<Asked>,
    /// The definitions this shell gave egui: egui's defaults with the arrows' fallback (`definitions`) and every face loaded.
    defs: Option<egui::FontDefinitions>,
    /// The families loaded, each with the pass after which egui has it bound.
    added: BTreeMap<String, u64>,
    loaded_bytes: u64,
    /// The fallback families loaded, by family, each with the font key it was given to egui under.
    fallbacks: BTreeMap<String, String>,
    /// The fallback keys behind every family, in the order they are there.
    fallback_order: Vec<String>,
    /// Bumped whenever the scan reports, a face loads or fails, or the worker stops: the
    /// fallbacks are worked out again only when this or the language has changed.
    generation: u64,
    /// The language and generation the fallbacks were last worked out for.
    fallback_seen: Option<(&'static str, u64)>,
    /// Started without a source: no installed font will ever be offered (the web host).
    no_source: bool,
    /// What the log has said about the fallbacks, so each cause is said once.
    said: BTreeSet<String>,
    /// The language whose missing script was last noticed in the log, and one still to log.
    noticed: Option<&'static str>,
    notice: Option<&'static str>,
}

/// The notice the log gives when `code`'s script has no face, in English like the rest of the log.
pub(crate) fn unserved_message(code: &str) -> String {
    let name = crate::i18n::LANGUAGES.iter().find(|l| l.code == code).map_or(code, |l| l.native_name);
    format!("{name} ({code}) needs a font this computer does not have: its text shows as boxes until one is installed.")
}

/// Why the language's script has no face to draw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unserved {
    /// The host offers no installed fonts (the web).
    NoSource,
    /// No installed font draws the script.
    NoneInstalled,
    /// Fonts for it are installed but none could be loaded: past the limits of loaded fonts, or
    /// not a font egui draws.
    Refused,
}

/// egui's bundled fonts with Hack, its monospace face, as the proportional text's first fallback:
/// the proportional face has no arrows (→, ←, ↔) and Hack does, so the design's strings draw no
/// missing glyph, on the web too. The design's own families (`ux`) are added on
/// top, so every font set the shell installs draws them.
pub(crate) fn definitions() -> egui::FontDefinitions {
    let mut defs = egui::FontDefinitions::default();
    if defs.font_data.contains_key("Hack")
        && let Some(chain) = defs.families.get_mut(&FontFamily::Proportional)
        && !chain.iter().any(|f| f == "Hack")
    {
        chain.insert(1.min(chain.len()), "Hack".to_string());
    }
    crate::ux::install(&mut defs);
    defs
}

/// Puts the fallback keys `order` behind every family of `defs`, taking out the ones `before` put
/// there, so each family draws a character its own faces lack from the first fallback that has it.
/// A family's own face, first in its chain, stays first.
fn arrange(defs: &mut egui::FontDefinitions, before: &[String], order: &[String]) {
    for chain in defs.families.values_mut() {
        let head = chain.first().cloned();
        let rest = chain.iter().skip(1).filter(|k| !before.contains(k) && !order.contains(k));
        let behind = order.iter().filter(|k| Some(*k) != head.as_ref());
        *chain = head.iter().chain(rest).chain(behind).cloned().collect();
    }
}

/// How far a face is from a regular one: upright first, then weight, then width.
fn irregularity(e: &FontEntry) -> u32 {
    u32::from(e.italic) * 100_000 + u32::from(e.weight.abs_diff(400)) + u32::from(e.stretch.abs_diff(5)) * 1000
}

/// How far down, as a fraction of the font size, a fallback face's glyphs move so its baseline is
/// the reference face's: egui centres a fallback face's line height on the first face's rather
/// than sharing its baseline, which sets Japanese and Chinese text above the Latin text and the
/// digits beside it. Zero when either face's metrics cannot be read; at most half the size.
pub(crate) fn baseline_shift(face: &[u8], index: u32, reference: &[u8], reference_index: u32) -> f32 {
    use skrifa::MetadataProvider;
    use skrifa::instance::{LocationRef, Size};
    let metrics = |bytes: &[u8], index: u32| {
        let m = skrifa::FontRef::from_index(bytes, index).ok()?.metrics(Size::unscaled(), LocationRef::default());
        let em = f32::from(m.units_per_em.max(1));
        Some((m.ascent / em, (m.ascent - m.descent + m.leading) / em))
    };
    let (Some((ascent, height)), Some((ref_ascent, ref_height))) = (metrics(face, index), metrics(reference, reference_index)) else { return 0.0 };
    let shift = ref_ascent - ascent - 0.5 * (ref_height - height);
    if shift.is_finite() { shift.clamp(-0.5, 0.5) } else { 0.0 }
}

/// Whether egui can load and draw a face: the parser egui loads fonts with takes it (the only
/// refusal behind egui's loading panic), and the tables it draws text from read.
pub(crate) fn readable(bytes: &[u8], index: u32) -> bool {
    use skrifa::raw::TableProvider;
    skrifa::FontRef::from_index(bytes, index).is_ok_and(|f| f.head().is_ok() && f.hhea().is_ok() && f.maxp().is_ok() && f.cmap().is_ok())
}

impl FontState {
    /// Once per frame: starts the worker on the first frame with a source, and takes in what it
    /// has scanned or read.
    pub(crate) fn frame(&mut self, source: Option<&Arc<dyn FontSource>>, ctx: &egui::Context) {
        if !self.started {
            self.started = true;
            let defs = self.defs.get_or_insert_with(definitions);
            ctx.set_fonts(defs.clone());
            if let Some(source) = source {
                self.start(Arc::clone(source), ctx);
            } else {
                self.no_source = true;
            }
        }
        let Some(done) = &self.done else {
            self.fallback(ctx, crate::i18n::language());
            return;
        };
        let mut events = Vec::new();
        let mut stopped = false;
        loop {
            match done.try_recv() {
                Ok(event) => events.push(event),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    stopped = true;
                    break;
                }
            }
        }
        self.take(ctx, events);
        if stopped {
            // A worker that stopped leaves what it was asked for unloaded, as failed.
            self.done = None;
            self.jobs = None;
            self.scanned = true;
            self.generation += 1;
            let asked = self.asked.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner);
            let in_flight = std::mem::take(&mut asked.in_flight);
            asked.failed.extend(in_flight);
        }
        self.fallback(ctx, crate::i18n::language());
    }

    fn take(&mut self, ctx: &egui::Context, events: Vec<Done>) {
        for event in events {
            match event {
                Done::Scanned(table) => {
                    self.scanned = true;
                    self.table = table;
                }
                Done::Loaded(family, index, bytes) => self.add(ctx, family, index, bytes),
            }
            self.generation += 1;
        }
    }

    /// Starts the one worker: it scans, then reads the faces asked for. It is never joined, so a
    /// scan stuck on a network mount cannot hold the exit; a panic in a job is caught there.
    fn start(&mut self, source: Arc<dyn FontSource>, ctx: &egui::Context) {
        let (jobs, job_rx) = mpsc::channel::<Job>();
        let (done_tx, done) = mpsc::channel::<Done>();
        let ctx = ctx.clone();
        let face_cap = self.caps.face;
        let worker = move || {
            let scanned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| table(source.scan(face_cap)))).unwrap_or_else(|_| {
                log::warn!("the installed fonts could not be scanned; no installed font is loaded");
                Table::default()
            });
            if done_tx.send(Done::Scanned(scanned)).is_err() {
                return;
            }
            ctx.request_repaint();
            while let Ok(Job::Load(family, entry)) = job_rx.recv() {
                let read = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| source.read(&entry, face_cap)))
                    .unwrap_or_else(|_| Err("the font reader stopped".to_string()));
                if done_tx.send(Done::Loaded(family, entry.index, read)).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
        };
        // A host that cannot start a thread has no installed fonts.
        match std::thread::Builder::new().name("archforge-fonts".into()).stack_size(8 << 20).spawn(worker) {
            Ok(_) => {
                self.jobs = Some(jobs);
                self.done = Some(done);
            }
            Err(e) => log::warn!("the font worker could not start ({e}); no installed font is loaded"),
        }
    }

    /// The canonical family a fallback list's name gives, when it is installed.
    fn resolve(&self, name: &str) -> Option<&str> {
        let name = name.trim();
        if name.is_empty() || self.table.names.is_empty() {
            return None;
        }
        self.table.names.get(&name.to_lowercase()).map(String::as_str)
    }

    /// Asks the worker for a family's regular face, once; past a cap, or without a worker, the
    /// family is marked failed and never loaded.
    fn request(&self, family: &str) {
        let Ok(mut asked) = self.asked.lock() else { return };
        if asked.in_flight.contains(family) || asked.failed.contains(family) {
            return;
        }
        let Some(entry) = self.table.regular.get(family) else { return };
        let over = self.added.len() + asked.in_flight.len() >= self.caps.count
            || entry.len > self.caps.face
            || self.loaded_bytes.saturating_add(entry.len) > self.caps.total;
        let sent = !over && self.jobs.as_ref().is_some_and(|jobs| jobs.send(Job::Load(family.to_string(), entry.clone())).is_ok());
        if sent {
            asked.in_flight.insert(family.to_string());
        } else {
            log::warn!("font {family:?} is not loaded: past the limit of loaded fonts, or no font worker");
            asked.failed.insert(family.to_string());
        }
    }

    /// Takes in a read face: checked with the parser egui loads fonts with, so egui's loading
    /// cannot panic on it, kept for the process and put behind every family as a script fallback.
    /// A face refused is never asked for again.
    fn add(&mut self, ctx: &egui::Context, family: String, index: u32, bytes: Result<Vec<u8>, String>) {
        let (caps, loaded, count) = (self.caps, self.loaded_bytes, self.added.len());
        let fits = |b: &Vec<u8>| b.len() as u64 <= caps.face && loaded.saturating_add(b.len() as u64) <= caps.total && count < caps.count;
        let asked = self.asked.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner);
        asked.in_flight.remove(&family);
        let bytes = match bytes {
            Ok(b) if fits(&b) && readable(&b, index) => b,
            Ok(_) => {
                log::warn!("font {family:?} cannot be drawn: not a font egui reads, or past a limit");
                asked.failed.insert(family);
                return;
            }
            Err(e) => {
                log::warn!("font {family:?} cannot be read: {e}");
                asked.failed.insert(family);
                return;
            }
        };
        self.loaded_bytes = self.loaded_bytes.saturating_add(bytes.len() as u64);
        // Loaded faces are never removed, so the bytes live for the process (bounded by the
        // total cap) and egui borrows them instead of copying them on every rebuild.
        let data: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let defs = self.defs.get_or_insert_with(definitions);
        // Moved onto the bundled face's baseline, so it sits level with the Latin text beside it.
        let mut tweak = egui::FontTweak::default();
        if let Some(reference) = defs.families.get(&FontFamily::Proportional).and_then(|c| c.first()).and_then(|k| defs.font_data.get(k)) {
            tweak.y_offset_factor = baseline_shift(data, index, (*reference.font).as_ref(), reference.index);
        }
        let key = format!("fallback:{family}");
        defs.font_data.insert(key.clone(), Arc::new(egui::FontData { index, tweak, ..egui::FontData::from_static(data) }));
        self.fallbacks.insert(family.clone(), key);
        let order = self.fallback_keys(crate::i18n::language());
        let defs = self.defs.get_or_insert_with(definitions);
        arrange(defs, &self.fallback_order, &order);
        self.fallback_order = order;
        // The shell is the only caller of set_fonts; the change is egui's at the next pass.
        ctx.set_fonts(defs.clone());
        ctx.request_repaint();
        self.added.insert(family, ctx.cumulative_pass_nr());
    }

    /// The first installed family of a fallback list that has not failed to load.
    fn installed_fallback(&self, list: &[&str]) -> Option<String> {
        let failed = |f: &str| self.asked.lock().map(|a| a.failed.contains(f)).unwrap_or(false);
        list.iter().filter_map(|name| self.resolve(name)).find(|f| self.table.regular.contains_key(*f) && !failed(f)).map(str::to_string)
    }

    /// The loaded fallback keys in the order `language` wants them: its own script's first, then
    /// the others loaded.
    fn fallback_keys(&self, language: &str) -> Vec<String> {
        let mut order: Vec<String> = Vec::new();
        for list in fallback_lists(language).iter().chain([&JAPANESE_FALLBACKS, &CHINESE_FALLBACKS]) {
            for name in *list {
                if let Some(key) = self.resolve(name).and_then(|f| self.fallbacks.get(f))
                    && !order.contains(key)
                {
                    order.push(key.clone());
                }
            }
        }
        order
    }

    /// After the scan, whenever the language changes or a face loads or fails: asks for the
    /// fallback families the language wants, the next installed one when one fails, puts the
    /// loaded ones behind every family in the language's order, and says once, by its cause, a
    /// script no face draws.
    fn fallback(&mut self, ctx: &egui::Context, language: &'static str) {
        if !self.scanned && !self.no_source {
            return;
        }
        if self.fallback_seen == Some((language, self.generation)) {
            return;
        }
        self.fallback_seen = Some((language, self.generation));
        for list in fallback_lists(language) {
            // A list already served by a loaded family asks for nothing more; a family refused at
            // once (past a cap) is passed over for the next.
            for _ in 0..list.len() {
                if list.iter().filter_map(|n| self.resolve(n)).any(|f| self.fallbacks.contains_key(f)) {
                    break;
                }
                let waiting =
                    list.iter().filter_map(|n| self.resolve(n)).any(|f| self.asked.lock().map(|a| a.in_flight.contains(f)).unwrap_or(false));
                match self.installed_fallback(list) {
                    Some(family) if !waiting && !self.added.contains_key(&family) => self.request(&family),
                    _ => break,
                }
            }
        }
        self.say_unserved(language);
        if self.noticed != Some(language) {
            self.noticed = None;
            self.notice = None;
            if self.unserved(language).is_some() {
                self.noticed = Some(language);
                self.notice = Some(language);
            }
        }
        let order = self.fallback_keys(language);
        if order != self.fallback_order {
            let defs = self.defs.get_or_insert_with(definitions);
            arrange(defs, &self.fallback_order, &order);
            self.fallback_order = order;
            ctx.set_fonts(defs.clone());
            ctx.request_repaint();
        }
    }

    /// The scripts' families installed for `language`'s lists, each canonical name once.
    fn installed_for(&self, lists: &[&[&str]]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for f in lists.iter().flat_map(|l| l.iter()).filter_map(|n| self.resolve(n)) {
            if self.table.regular.contains_key(f) && !out.iter().any(|o| o == f) {
                out.push(f.to_string());
            }
        }
        out
    }

    /// Whether the scan or a load is still to report.
    fn pending(&self) -> bool {
        !self.no_source && (!self.scanned || self.asked.lock().map(|a| !a.in_flight.is_empty()).unwrap_or(false))
    }

    /// Why no face draws `language`'s script; none for a language in the bundled faces' scripts,
    /// one a loaded face draws, or while that is not known yet (the scan or a load pending).
    /// Japanese is drawn by a Japanese or a Chinese face (which has the kana), Simplified Chinese
    /// only by a Chinese one.
    pub(crate) fn unserved(&self, language: &str) -> Option<Unserved> {
        let lists: &[&[&str]] = match language {
            "ja" => &[JAPANESE_FALLBACKS, CHINESE_FALLBACKS],
            "zh-Hans" => &[CHINESE_FALLBACKS],
            _ => return None,
        };
        if self.no_source {
            return Some(Unserved::NoSource);
        }
        let installed = self.installed_for(lists);
        if self.pending() || installed.iter().any(|f| self.fallbacks.contains_key(f)) {
            return None;
        }
        Some(if installed.is_empty() { Unserved::NoneInstalled } else { Unserved::Refused })
    }

    /// The language whose script no face draws, once after it is chosen or started in, for the
    /// shell to log.
    pub(crate) fn take_notice(&mut self) -> Option<&'static str> {
        self.notice.take()
    }

    /// Says in the log, once each, why a script the language shows has no face, by its cause: no
    /// installed fonts at all (the web), none for the script, fonts refused at a limit, or only
    /// Japanese unserved, drawn in the Chinese face's forms.
    fn say_unserved(&mut self, language: &str) {
        if self.pending() {
            return;
        }
        let (script, lists): (&str, &[&[&str]]) = match language {
            "ja" => ("Japanese", &[JAPANESE_FALLBACKS, CHINESE_FALLBACKS]),
            "zh-Hans" => ("Simplified Chinese", &[CHINESE_FALLBACKS]),
            // Names in those scripts, which a Chinese face draws.
            _ => ("Japanese or Chinese", &[CHINESE_FALLBACKS, JAPANESE_FALLBACKS]),
        };
        let installed = self.installed_for(lists);
        let loaded = |list: &[&str]| list.iter().filter_map(|n| self.resolve(n)).any(|f| self.fallbacks.contains_key(f));
        let message = if self.no_source {
            Some(format!("this host offers no installed fonts: {script} text shows as boxes"))
        } else if installed.is_empty() {
            Some(format!("no installed font draws {script}; such text shows as boxes until one is installed (for example Noto Sans CJK)"))
        } else if !lists.iter().any(|l| loaded(l)) {
            Some(format!(
                "the installed {script} fonts ({}) could not be loaded: past the limits of loaded fonts, or not a font egui draws; such text shows as boxes",
                installed.join(", ")
            ))
        } else if language == "ja" && !loaded(JAPANESE_FALLBACKS) {
            Some("no Japanese font is installed or loadable; Japanese text takes the Chinese font's character forms".to_string())
        } else {
            None
        };
        if let Some(message) = message
            && self.said.insert(message.clone())
        {
            log::warn!("{message}");
        }
    }

    /// The fallback keys behind every family, in order (tests only).
    #[cfg(test)]
    pub(crate) fn fallback_order(&self) -> &[String] {
        &self.fallback_order
    }

    /// Whether the fonts are settled: scanned, nothing in flight, every face loaded bound in an
    /// earlier pass (tests only).
    #[cfg(test)]
    pub(crate) fn settled(&self, ctx: &egui::Context) -> bool {
        if self.done.is_none() {
            return true;
        }
        let pass = ctx.cumulative_pass_nr();
        self.scanned && self.asked.lock().map(|a| a.in_flight.is_empty()).unwrap_or(true) && self.added.values().all(|p| pass > *p)
    }

    /// Waits up to `timeout` for the worker to report, taking in what it sends (tests only).
    #[cfg(test)]
    pub(crate) fn wait(&mut self, ctx: &egui::Context, timeout: std::time::Duration) {
        let Some(done) = &self.done else { return };
        let mut events = Vec::new();
        if let Ok(event) = done.recv_timeout(timeout) {
            events.push(event);
            while let Ok(event) = done.try_recv() {
                events.push(event);
            }
        }
        self.take(ctx, events);
    }

    /// Whether a family is waiting on the worker (tests only).
    #[cfg(test)]
    pub(crate) fn in_flight(&self) -> usize {
        self.asked.lock().map(|a| a.in_flight.len()).unwrap_or(0)
    }
}
