//! Scorer: matches reader events to the answer key and computes the metrics, the reports and
//! the baseline comparison.

use crate::background::BgTextMap;
use crate::model::{ItemKind, TextItem};
use crate::reader::ReadEvent;
use crate::runner::ScenarioRun;
use crate::text::{cer_normalized, normalize};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;

/// A read matches an item when its CER is at most this...
pub const MAX_MATCH_CER: f64 = 0.35;
/// ...and it happens between the item's start and this long after its end.
pub const LATE_WINDOW_MS: f64 = 1500.0;
/// An unmatched read whose rect lies at least this much inside a background text region is a
/// background read.
pub const BG_OVERLAP: f32 = 0.5;

/// Duration buckets, by readable time (`TextItem::readable_ms`: fades and typing excluded).
pub const BUCKETS: &[(&str, f64, f64)] =
    &[("<1s", 0.0, 1000.0), ("1-2s", 1000.0, 2000.0), ("2-4s", 2000.0, 4000.0), (">4s", 4000.0, f64::INFINITY)];

/// What one event was judged to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// First read of a should_read item.
    Read(usize),
    /// First read of a prompt (optional reading).
    PromptRead(usize),
    /// A further read of an item that was already read.
    Duplicate(usize),
    /// Read of a should_read = false item (HUD, damage number, menu).
    Noise(usize),
    /// Matches no item but lies on text that is part of the background frames.
    Background,
    /// Matches nothing.
    Hallucination,
}

fn is_prompt(it: &TextItem) -> bool {
    it.kind == ItemKind::Prompt
}

/// Truth shorter than this (normalised chars) must be read exactly: one edit in "Ash" is
/// already CER 0.33, so "Ask" would otherwise count as a read of the speaker "Ash".
pub const SHORT_TRUTH_CHARS: usize = 5;

/// CER of a normalised read against a normalised truth when it is close enough to count as a
/// read of it, else `None`. An empty read (only punctuation or spaces) matches nothing.
pub fn match_cer(read: &str, truth: &str) -> Option<f64> {
    if read.is_empty() || truth.is_empty() {
        return None;
    }
    let c = cer_normalized(read, truth);
    let short = truth.chars().count() < SHORT_TRUTH_CHARS;
    if c > MAX_MATCH_CER || (short && c > 0.0) { None } else { Some(c) }
}

/// Judges every event of one scene.
///
/// Each event only competes for its best items: the items with its lowest CER, and of those only
/// the ones on screen at the event time when any is (a read of a toast that just replaced another
/// is about the new one). Inside those sets events and items are matched one-to-one with a
/// maximum matching, so two identical toasts shown together can both be read. An event left
/// over is a further read of its best item: a duplicate, or noise when that item is not to be read.
pub fn match_events(items: &[TextItem], events: &[ReadEvent], bg: &BgTextMap) -> Vec<Verdict> {
    let item_norm: Vec<String> = items.iter().map(|i| normalize(&i.text)).collect();
    // Per event: its best items, most recently started first.
    let mut best: Vec<Vec<usize>> = Vec::with_capacity(events.len());
    for ev in events {
        let en = normalize(&ev.text);
        let mut cands: Vec<(f64, usize)> = Vec::new();
        for (i, it) in items.iter().enumerate() {
            if ev.t_ms < it.start_ms || ev.t_ms > it.end_ms + LATE_WINDOW_MS {
                continue;
            }
            if let Some(c) = match_cer(&en, &item_norm[i]) {
                cands.push((c, i));
            }
        }
        let Some(lo) = cands.iter().map(|c| c.0).min_by(f64::total_cmp) else {
            best.push(Vec::new());
            continue;
        };
        let mut set: Vec<usize> = cands.iter().filter(|c| c.0 <= lo + 1e-9).map(|c| c.1).collect();
        if set.iter().any(|&i| items[i].visible_at(ev.t_ms)) {
            set.retain(|&i| items[i].visible_at(ev.t_ms));
        }
        set.sort_by(|&a, &b| (ev.t_ms - items[a].start_ms).total_cmp(&(ev.t_ms - items[b].start_ms)).then(a.cmp(&b)));
        best.push(set);
    }

    // Maximum one-to-one matching (augmenting paths), events in time order.
    let mut order: Vec<usize> = (0..events.len()).collect();
    order.sort_by(|&a, &b| events[a].t_ms.total_cmp(&events[b].t_ms).then(a.cmp(&b)));
    let mut item_event: Vec<Option<usize>> = vec![None; items.len()];
    for &e in &order {
        let mut seen = vec![false; items.len()];
        augment(e, &best, &mut item_event, &mut seen);
    }
    let mut event_item: Vec<Option<usize>> = vec![None; events.len()];
    for (i, e) in item_event.iter().enumerate() {
        if let Some(e) = e {
            event_item[*e] = Some(i);
        }
    }

    (0..events.len())
        .map(|e| {
            if let Some(i) = event_item[e] {
                let it = &items[i];
                return if it.should_read {
                    Verdict::Read(i)
                } else if is_prompt(it) {
                    Verdict::PromptRead(i)
                } else {
                    Verdict::Noise(i)
                };
            }
            if let Some(&i) = best[e].first() {
                let it = &items[i];
                return if it.should_read || is_prompt(it) { Verdict::Duplicate(i) } else { Verdict::Noise(i) };
            }
            if on_background_text(&events[e], bg) { Verdict::Background } else { Verdict::Hallucination }
        })
        .collect()
}

/// Kuhn's augmenting path step: tries to give event `e` one of its best items.
fn augment(e: usize, best: &[Vec<usize>], item_event: &mut [Option<usize>], seen: &mut [bool]) -> bool {
    for &i in &best[e] {
        if seen[i] {
            continue;
        }
        seen[i] = true;
        let free = match item_event[i] {
            None => true,
            Some(other) => augment(other, best, item_event, seen),
        };
        if free {
            item_event[i] = Some(e);
            return true;
        }
    }
    false
}

/// True when an unmatched event lies on background text shown in the last `LATE_WINDOW_MS`.
/// With a rect: it must lie mostly inside a region. Without one: its text must match a known
/// string of an active region.
pub fn on_background_text(ev: &ReadEvent, bg: &BgTextMap) -> bool {
    if bg.is_empty() {
        return false;
    }
    let en = normalize(&ev.text);
    let mut t = ev.t_ms;
    while t >= ev.t_ms - LATE_WINDOW_MS {
        for (rect, texts) in bg.active_at(t) {
            match &ev.rect {
                Some(r) => {
                    if r.area() > 0.0 && rect.intersection(r) / r.area() >= BG_OVERLAP {
                        return true;
                    }
                }
                None => {
                    // Text-only rule needs 3+ chars, so junk like "1" or "N" stays a hallucination.
                    if en.chars().count() >= 3 && texts.iter().any(|s| cer_normalized(&en, &normalize(s)) <= MAX_MATCH_CER) {
                        return true;
                    }
                }
            }
        }
        t -= bg.frame_ms().min(250.0);
    }
    false
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Bucket {
    pub name: String,
    pub total: usize,
    pub read: usize,
    pub recall: f64,
}

/// Scene or suite totals.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Metrics {
    pub scenarios: usize,
    pub items_should_read: usize,
    pub items_read: usize,
    pub recall: f64,
    pub recall_by_duration: Vec<Bucket>,
    /// Mean CER (normalised text) of the first read of each read item.
    pub mean_cer: f64,
    /// Share of read items whose first read is exact after normalising.
    pub exact_match_rate: f64,
    pub prompts_total: usize,
    pub prompts_read: usize,
    pub prompt_recall: f64,
    pub events_total: usize,
    pub duplicates: usize,
    pub noise_reads: usize,
    pub background_reads: usize,
    pub hallucinations: usize,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub cpu_ms_p50: f64,
    pub cpu_ms_p95: f64,
    pub frames_total: usize,
    pub frames_processed: usize,
}

/// Raw numbers kept per scene so suite totals are exact (not means of means).
#[derive(Clone, Debug, Default)]
struct Raw {
    should: usize,
    read: usize,
    bucket_total: [usize; 4],
    bucket_read: [usize; 4],
    /// Items whose recorded times are not exact; they are in no duration bucket.
    unsure_total: usize,
    unsure_read: usize,
    cers: Vec<f64>,
    latencies: Vec<f64>,
    prompts: usize,
    prompts_read: usize,
    events: usize,
    dups: usize,
    noise: usize,
    bg: usize,
    halluc: usize,
    costs: Vec<f64>,
    frames_total: usize,
    frames_processed: usize,
    scenarios: usize,
}

impl Raw {
    fn add(&mut self, o: &Raw) {
        self.should += o.should;
        self.read += o.read;
        for k in 0..4 {
            self.bucket_total[k] += o.bucket_total[k];
            self.bucket_read[k] += o.bucket_read[k];
        }
        self.unsure_total += o.unsure_total;
        self.unsure_read += o.unsure_read;
        self.cers.extend(&o.cers);
        self.latencies.extend(&o.latencies);
        self.prompts += o.prompts;
        self.prompts_read += o.prompts_read;
        self.events += o.events;
        self.dups += o.dups;
        self.noise += o.noise;
        self.bg += o.bg;
        self.halluc += o.halluc;
        self.costs.extend(&o.costs);
        self.frames_total += o.frames_total;
        self.frames_processed += o.frames_processed;
        self.scenarios += o.scenarios;
    }

    fn metrics(&self) -> Metrics {
        let ratio = |a: usize, b: usize| if b == 0 { 0.0 } else { a as f64 / b as f64 };
        let mean = |v: &[f64]| if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 };
        Metrics {
            scenarios: self.scenarios,
            items_should_read: self.should,
            items_read: self.read,
            recall: ratio(self.read, self.should),
            recall_by_duration: BUCKETS
                .iter()
                .enumerate()
                .map(|(k, (name, _, _))| Bucket {
                    name: name.to_string(),
                    total: self.bucket_total[k],
                    read: self.bucket_read[k],
                    recall: ratio(self.bucket_read[k], self.bucket_total[k]),
                })
                .chain((self.unsure_total > 0).then(|| Bucket {
                    name: UNSURE_BUCKET.to_string(),
                    total: self.unsure_total,
                    read: self.unsure_read,
                    recall: ratio(self.unsure_read, self.unsure_total),
                }))
                .collect(),
            mean_cer: mean(&self.cers),
            exact_match_rate: ratio(self.cers.iter().filter(|c| **c == 0.0).count(), self.cers.len()),
            prompts_total: self.prompts,
            prompts_read: self.prompts_read,
            prompt_recall: ratio(self.prompts_read, self.prompts),
            events_total: self.events,
            duplicates: self.dups,
            noise_reads: self.noise,
            background_reads: self.bg,
            hallucinations: self.halluc,
            latency_p50_ms: percentile(&self.latencies, 0.5),
            latency_p95_ms: percentile(&self.latencies, 0.95),
            cpu_ms_p50: percentile(&self.costs, 0.5),
            cpu_ms_p95: percentile(&self.costs, 0.95),
            frames_total: self.frames_total,
            frames_processed: self.frames_processed,
        }
    }
}

/// Nearest-rank percentile; 0 for an empty list.
pub fn percentile(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let k = ((p * s.len() as f64).ceil() as usize).clamp(1, s.len()) - 1;
    s[k]
}

fn bucket_of(d: f64) -> usize {
    BUCKETS.iter().position(|(_, lo, hi)| d >= *lo && d < *hi).unwrap_or(3)
}

/// Bucket name of recorded items whose start or end time is not exact.
pub const UNSURE_BUCKET: &str = "time unsure";

/// Recorded truth ids whose times are not exact (see `RecordedScenario`).
#[derive(Clone, Debug, Default)]
pub struct TimingFlags {
    /// Start not exact: no latency, no duration bucket.
    pub start: HashSet<String>,
    /// End not exact: no duration bucket.
    pub end: HashSet<String>,
}

impl TimingFlags {
    pub fn new(start: &[String], end: &[String]) -> Self {
        TimingFlags { start: start.iter().cloned().collect(), end: end.iter().cloned().collect() }
    }
}

/// Per-scene result in the report.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct ScenarioScore {
    pub id: String,
    #[serde(default)]
    pub genres: Vec<String>,
    pub metrics: Metrics,
    /// Ids and texts of should_read items nobody read.
    pub missed: Vec<String>,
    /// Texts of noise reads and hallucinations (first 20 of each), for debugging.
    pub false_reads: Vec<String>,
    #[serde(default)]
    pub background_read_texts: Vec<String>,
}

pub struct Scored {
    pub score: ScenarioScore,
    raw: Raw,
}

pub fn score_scenario(id: &str, genres: &[String], items: &[TextItem], run: &ScenarioRun, bg: &BgTextMap) -> Scored {
    score_scenario_with(id, genres, items, run, bg, &TimingFlags::default())
}

/// Scores one scene. Latency is measured from the time the text is fully readable (after fade-in
/// and typing); a read before that counts as 0 ms.
pub fn score_scenario_with(id: &str, genres: &[String], items: &[TextItem], run: &ScenarioRun, bg: &BgTextMap, timing: &TimingFlags) -> Scored {
    let verdicts = match_events(items, &run.events, bg);
    let mut raw = Raw { scenarios: 1, events: run.events.len(), costs: run.frame_costs_ms.clone(), frames_total: run.frames_total, frames_processed: run.frames_processed, ..Default::default() };
    let mut read = vec![false; items.len()];
    let mut false_reads = Vec::new();
    let mut bg_texts = Vec::new();
    for (ev, v) in run.events.iter().zip(&verdicts) {
        match *v {
            Verdict::Read(i) => {
                read[i] = true;
                raw.cers.push(cer_normalized(&normalize(&ev.text), &normalize(&items[i].text)));
                if !timing.start.contains(&items[i].id) {
                    raw.latencies.push((ev.t_ms - items[i].readable_from_ms()).max(0.0));
                }
            }
            Verdict::PromptRead(_) => raw.prompts_read += 1,
            Verdict::Duplicate(_) => raw.dups += 1,
            Verdict::Noise(_) => {
                raw.noise += 1;
                false_reads.push(format!("noise @{:.0}: {}", ev.t_ms, ev.text));
            }
            Verdict::Background => {
                raw.bg += 1;
                bg_texts.push(format!("@{:.0}: {}", ev.t_ms, ev.text));
            }
            Verdict::Hallucination => {
                raw.halluc += 1;
                false_reads.push(format!("hallucination @{:.0}: {}", ev.t_ms, ev.text));
            }
        }
    }
    let mut missed = Vec::new();
    for (i, it) in items.iter().enumerate() {
        if it.should_read {
            raw.should += 1;
            let unsure = timing.start.contains(&it.id) || timing.end.contains(&it.id);
            let (total, done) = if unsure {
                (&mut raw.unsure_total, &mut raw.unsure_read)
            } else {
                let b = bucket_of(it.readable_ms());
                (&mut raw.bucket_total[b], &mut raw.bucket_read[b])
            };
            *total += 1;
            if read[i] {
                raw.read += 1;
                *done += 1;
            } else {
                missed.push(format!("{} ({:.0} ms readable): {}", it.id, it.readable_ms(), it.text.replace('\n', " ")));
            }
        } else if is_prompt(it) {
            raw.prompts += 1;
        }
    }
    false_reads.truncate(40);
    bg_texts.truncate(20);
    Scored {
        score: ScenarioScore { id: id.to_string(), genres: genres.to_vec(), metrics: raw.metrics(), missed, false_reads, background_read_texts: bg_texts },
        raw,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Report {
    pub suite: String,
    /// Seed of the generated suite (`None` for the recorded set and live runs).
    #[serde(default)]
    pub seed: Option<u64>,
    /// Fingerprint of every scene's answer key, in order (see `suite_fingerprint`).
    #[serde(default)]
    pub truth_fingerprint: String,
    pub reader: String,
    #[serde(default)]
    pub reader_options: BTreeMap<String, String>,
    pub clock: String,
    pub totals: Metrics,
    pub scenarios: Vec<ScenarioScore>,
}

pub fn build_report(suite: &str, reader: &str, clock: &str, scored: Vec<Scored>) -> Report {
    let mut total = Raw::default();
    for s in &scored {
        total.add(&s.raw);
    }
    Report {
        suite: suite.to_string(),
        reader: reader.to_string(),
        clock: clock.to_string(),
        totals: total.metrics(),
        scenarios: scored.into_iter().map(|s| s.score).collect(),
        ..Default::default()
    }
}

/// One fingerprint for a whole set of scenes: FNV-1a over `id:fingerprint` lines in order.
pub fn suite_fingerprint(scenes: &[(String, String)]) -> String {
    let joined: String = scenes.iter().map(|(id, f)| format!("{id}:{f}\n")).collect();
    crate::model::fnv_hex(joined.as_bytes())
}

/// Reasons a report cannot be compared with a baseline: they were not made on the same scenes
/// and answer keys. Empty when they match.
pub fn baseline_mismatch(cur: &Report, base: &Report) -> Vec<String> {
    let mut v = Vec::new();
    if cur.suite != base.suite {
        v.push(format!("suite {:?} vs baseline {:?}", cur.suite, base.suite));
    }
    if cur.seed != base.seed {
        v.push(format!("seed {:?} vs baseline {:?}", cur.seed, base.seed));
    }
    let ids = |r: &Report| r.scenarios.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
    if ids(cur) != ids(base) {
        v.push(format!("{} scenes vs baseline {} (or different ids)", cur.scenarios.len(), base.scenarios.len()));
    }
    if cur.totals.items_should_read != base.totals.items_should_read {
        v.push(format!("{} items to read vs baseline {}", cur.totals.items_should_read, base.totals.items_should_read));
    }
    if base.truth_fingerprint.is_empty() {
        v.push("baseline has no truth fingerprint (made by an older arena); make a new baseline".into());
    } else if cur.truth_fingerprint != base.truth_fingerprint {
        v.push(format!("answer key fingerprint {} vs baseline {}", cur.truth_fingerprint, base.truth_fingerprint));
    }
    v
}

/// Reader label with its options, e.g. `winocr-naive [engine=ppocr]`.
pub fn reader_label(reader: &str, opts: &BTreeMap<String, String>) -> String {
    if opts.is_empty() {
        reader.to_string()
    } else {
        format!("{reader} [{}]", opts.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", "))
    }
}

pub fn report_markdown(r: &Report) -> String {
    let m = &r.totals;
    let mut s = String::new();
    let _ = writeln!(s, "# Arena report: {} / {} / {} clock\n", r.suite, reader_label(&r.reader, &r.reader_options), r.clock);
    if let Some(seed) = r.seed {
        let _ = writeln!(s, "Seed {seed}, answer key fingerprint `{}`.\n", r.truth_fingerprint);
    } else if !r.truth_fingerprint.is_empty() {
        let _ = writeln!(s, "Answer key fingerprint `{}`.\n", r.truth_fingerprint);
    }
    let _ = writeln!(s, "| Metric | Value |\n|---|---|");
    let rows: Vec<(&str, String)> = vec![
        ("Scenarios", m.scenarios.to_string()),
        ("Recall", format!("{:.3} ({}/{})", m.recall, m.items_read, m.items_should_read)),
        ("Mean CER", format!("{:.3}", m.mean_cer)),
        ("Exact-match rate", format!("{:.3}", m.exact_match_rate)),
        ("Prompt recall", format!("{:.3} ({}/{})", m.prompt_recall, m.prompts_read, m.prompts_total)),
        ("Events", m.events_total.to_string()),
        ("Duplicates", m.duplicates.to_string()),
        ("Noise reads", m.noise_reads.to_string()),
        ("Background reads", m.background_reads.to_string()),
        ("Hallucinations", m.hallucinations.to_string()),
        ("Latency p50 / p95 ms", format!("{:.0} / {:.0}", m.latency_p50_ms, m.latency_p95_ms)),
        ("CPU ms per frame p50 / p95", format!("{:.2} / {:.2}", m.cpu_ms_p50, m.cpu_ms_p95)),
        ("Frames processed / total", format!("{} / {}", m.frames_processed, m.frames_total)),
    ];
    for (k, v) in rows {
        let _ = writeln!(s, "| {k} | {v} |");
    }
    let _ = writeln!(s, "\n## Recall by readable time (fades and typing left out)\n\n| Duration | Read | Total | Recall |\n|---|---|---|---|");
    for b in &m.recall_by_duration {
        let _ = writeln!(s, "| {} | {} | {} | {:.3} |", b.name, b.read, b.total, b.recall);
    }
    let _ = writeln!(s, "\n## Scenarios\n\n| Id | Genres | Recall | CER | Dup | Noise | Bg | Halluc |\n|---|---|---|---|---|---|---|---|");
    for sc in &r.scenarios {
        let x = &sc.metrics;
        let _ = writeln!(
            s,
            "| {} | {} | {:.2} ({}/{}) | {:.3} | {} | {} | {} | {} |",
            sc.id,
            sc.genres.join(", "),
            x.recall,
            x.items_read,
            x.items_should_read,
            x.mean_cer,
            x.duplicates,
            x.noise_reads,
            x.background_reads,
            x.hallucinations
        );
    }
    let missed: Vec<_> = r.scenarios.iter().flat_map(|sc| sc.missed.iter().map(move |m| (sc.id.clone(), m))).take(60).collect();
    if !missed.is_empty() {
        let _ = writeln!(s, "\n## Missed items (first 60)\n");
        for (id, m) in missed {
            let _ = writeln!(s, "- {id}: {m}");
        }
    }
    s
}

/// Allowed worsening before `--baseline` fails.
#[derive(Clone, Copy, Debug)]
pub struct Tolerance {
    pub recall: f64,
    pub cer: f64,
    /// A count may grow by max(abs, rel * baseline).
    pub count_abs: usize,
    pub count_rel: f64,
}

impl Default for Tolerance {
    fn default() -> Self {
        Tolerance { recall: 0.01, cer: 0.01, count_abs: 2, count_rel: 0.10 }
    }
}

pub struct Comparison {
    pub lines: Vec<String>,
    pub regressions: Vec<String>,
}

/// Compares current totals with a baseline. Regressions: recall, CER, duplicates, noise,
/// hallucinations. Background reads, prompts, latency and CPU are shown only.
pub fn compare(cur: &Metrics, base: &Metrics, tol: Tolerance) -> Comparison {
    let mut lines = Vec::new();
    let mut regressions = Vec::new();
    let mut f = |name: &str, c: f64, b: f64, higher_is_worse: bool, limit: Option<f64>| {
        let d = c - b;
        lines.push(format!("{name}: {b:.3} -> {c:.3} ({d:+.3})"));
        if let Some(lim) = limit {
            let worse = if higher_is_worse { d } else { -d };
            if worse > lim {
                regressions.push(format!("{name} worse by {worse:.3} (tolerance {lim:.3})"));
            }
        }
    };
    let count_lim = |b: usize| (tol.count_abs as f64).max(tol.count_rel * b as f64);
    f("recall", cur.recall, base.recall, false, Some(tol.recall));
    f("mean_cer", cur.mean_cer, base.mean_cer, true, Some(tol.cer));
    f("noise_reads", cur.noise_reads as f64, base.noise_reads as f64, true, Some(count_lim(base.noise_reads)));
    f("hallucinations", cur.hallucinations as f64, base.hallucinations as f64, true, Some(count_lim(base.hallucinations)));
    f("background_reads", cur.background_reads as f64, base.background_reads as f64, true, None);
    f("duplicates", cur.duplicates as f64, base.duplicates as f64, true, Some(count_lim(base.duplicates)));
    f("prompt_recall", cur.prompt_recall, base.prompt_recall, false, None);
    f("latency_p50_ms", cur.latency_p50_ms, base.latency_p50_ms, true, None);
    f("cpu_ms_p50", cur.cpu_ms_p50, base.cpu_ms_p50, true, None);
    Comparison { lines, regressions }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn item(id: &str, text: &str, kind: ItemKind, start: f64, end: f64) -> TextItem {
        TextItem {
            id: id.into(),
            text: text.into(),
            kind,
            should_read: kind.default_should_read(),
            rect: Rect::new(10.0, 10.0, 100.0, 20.0),
            start_ms: start,
            end_ms: end,
            style: Style {
                font: "Arial".into(),
                size_px: 20.0,
                color: [255; 4],
                outline: None,
                shadow: None,
                panel: None,
                bold: false,
                italic: false,
                align: Align::Left,
                line_spacing: 1.25,
            },
            anim: Anim::default(),
        }
    }

    fn ev(t: f64, text: &str) -> ReadEvent {
        ReadEvent { t_ms: t, text: text.into(), rect: None, zone: None }
    }

    #[test]
    fn replace_chain_matches_one_to_one() {
        let items = vec![
            item("a", "+ 6 Iron Ore (27)", ItemKind::PickupToast, 0.0, 1000.0),
            item("b", "+ 12 Iron Ore (33)", ItemKind::PickupToast, 1000.0, 3000.0),
        ];
        // Both reads land inside both windows; each must go to its own item.
        let events = vec![ev(1100.0, "+ 12 Iron Ore (33)"), ev(1050.0, "+ 6 Iron Ore (27)")];
        let v = match_events(&items, &events, &BgTextMap::default());
        assert_eq!(v, vec![Verdict::Read(1), Verdict::Read(0)]);
    }

    #[test]
    fn repeated_identical_toasts_and_duplicates() {
        let items = vec![
            item("a", "+ 1 Rope", ItemKind::PickupToast, 0.0, 1000.0),
            item("b", "+ 1 Rope", ItemKind::PickupToast, 2000.0, 3000.0),
        ];
        let events = vec![ev(100.0, "+ 1 Rope"), ev(200.0, "+1 Rope"), ev(2100.0, "+ 1 Rope")];
        let v = match_events(&items, &events, &BgTextMap::default());
        assert_eq!(v, vec![Verdict::Read(0), Verdict::Duplicate(0), Verdict::Read(1)]);
    }

    #[test]
    fn windows_noise_prompts_and_hallucinations() {
        let items = vec![
            item("d", "Follow the river", ItemKind::Dialogue, 1000.0, 2000.0),
            item("h", "HP 100/120", ItemKind::HudStatic, 0.0, 5000.0),
            item("p", "Press [E] to open", ItemKind::Prompt, 0.0, 5000.0),
        ];
        let events = vec![
            ev(900.0, "Follow the river"),   // before start: no match
            ev(3600.0, "Follow the river"),  // after end + 1500: no match
            ev(3400.0, "Folow the rivr"),    // inside the late window, CER 0.125
            ev(100.0, "HP 100/120"),
            ev(150.0, "press e to open"),
            ev(200.0, "!!"),
        ];
        let v = match_events(&items, &events, &BgTextMap::default());
        assert_eq!(v[0], Verdict::Hallucination);
        assert_eq!(v[1], Verdict::Hallucination);
        assert_eq!(v[2], Verdict::Read(0));
        assert_eq!(v[3], Verdict::Noise(1));
        assert_eq!(v[4], Verdict::PromptRead(2));
        assert_eq!(v[5], Verdict::Hallucination);
    }

    #[test]
    fn metrics_and_buckets() {
        let items = vec![
            item("a", "short one", ItemKind::Subtitle, 0.0, 500.0),
            item("b", "long line here", ItemKind::Subtitle, 1000.0, 6000.0),
        ];
        let run = ScenarioRun {
            scenario_id: "x".into(),
            reader: "t".into(),
            clock: crate::runner::Clock::Ideal,
            events: vec![ev(1200.0, "long lime here")],
            frame_costs_ms: vec![1.0, 2.0, 3.0],
            frames_total: 3,
            frames_processed: 3,
            truth_fingerprint: String::new(),
            reader_options: Default::default(),
        };
        let s = score_scenario("x", &[], &items, &run, &BgTextMap::default());
        let m = &s.score.metrics;
        assert_eq!(m.recall, 0.5);
        assert_eq!(m.recall_by_duration[0].total, 1);
        assert_eq!(m.recall_by_duration[0].read, 0);
        assert_eq!(m.recall_by_duration[3].read, 1);
        assert!((m.mean_cer - 1.0 / 14.0).abs() < 1e-9);
        assert_eq!(m.exact_match_rate, 0.0);
        assert_eq!(m.latency_p50_ms, 200.0);
        assert_eq!(m.cpu_ms_p50, 2.0);
        assert_eq!(s.score.missed.len(), 1);
    }

    #[test]
    fn baseline_flags_regressions() {
        let base = Metrics { recall: 0.9, mean_cer: 0.05, noise_reads: 10, hallucinations: 3, ..Default::default() };
        let same = compare(&base, &base, Tolerance::default());
        assert!(same.regressions.is_empty());
        let worse = Metrics { recall: 0.85, mean_cer: 0.05, noise_reads: 12, hallucinations: 6, ..Default::default() };
        let c = compare(&worse, &base, Tolerance::default());
        assert_eq!(c.regressions.len(), 2, "{:?}", c.regressions); // recall and hallucinations
        let better = Metrics { recall: 0.95, mean_cer: 0.01, noise_reads: 0, hallucinations: 0, ..Default::default() };
        assert!(compare(&better, &base, Tolerance::default()).regressions.is_empty());
    }

    #[test]
    fn background_text_reads_are_their_own_bucket() {
        // One static region top-left with a version string, one region shown only on frames 5..=9.
        let bg = BgTextMap::new(
            vec![
                (Rect::new(0.0, 0.0, 200.0, 30.0), None, vec!["v1.2.5 CL#520140".into()]),
                (Rect::new(0.0, 400.0, 300.0, 100.0), Some([5, 9]), vec!["+ 9 Leaves".into()]),
            ],
            10.0,
            0.0,
            0,
            20,
        );
        let items = vec![item("d", "Follow the river", ItemKind::Dialogue, 0.0, 3000.0)];
        let with_rect = |t: f64, text: &str, r: Rect| ReadEvent { t_ms: t, text: text.into(), rect: Some(r), zone: None };
        let events = vec![
            with_rect(100.0, "v1.2.5 CL#520140", Rect::new(10.0, 5.0, 150.0, 20.0)), // on the static region
            ev(200.0, "V1.2.5 CL#52O14O"),                                            // no rect, known text
            ev(300.0, "Leaves"),                                                       // known text, region not shown yet
            with_rect(700.0, "+ 9 Leaves", Rect::new(20.0, 420.0, 120.0, 30.0)),       // region shown (frame 7)
            with_rect(700.0, "grass", Rect::new(600.0, 300.0, 100.0, 30.0)),           // texture misread elsewhere
            ev(800.0, "Follow the river"),                                             // the real line
        ];
        let v = match_events(&items, &events, &bg);
        assert_eq!(
            v,
            vec![
                Verdict::Background,
                Verdict::Background,
                Verdict::Hallucination,
                Verdict::Background,
                Verdict::Hallucination,
                Verdict::Read(0)
            ]
        );
    }

    fn judge(items: &[TextItem], events: &[ReadEvent]) -> Vec<Verdict> {
        match_events(items, events, &BgTextMap::default())
    }

    /// The four wrong-score cases of the 2026-11-21 review, plus the guard case.
    #[test]
    fn repeated_reads_never_credit_another_item() {
        // replace_dup: the text of b read twice; a was never read.
        let items = vec![
            item("a", "+ 6 Iron Ore (27)", ItemKind::PickupToast, 0.0, 1000.0),
            item("b", "+ 12 Iron Ore (33)", ItemKind::PickupToast, 1000.0, 3000.0),
        ];
        let v = judge(&items, &[ev(1100.0, "+ 12 Iron Ore (33)"), ev(1600.0, "+ 12 Iron Ore (33)")]);
        assert_eq!(v, vec![Verdict::Read(1), Verdict::Duplicate(1)]);

        // identical_dup: the second "Run!" read twice; the first one (ended) was missed.
        let items = vec![item("a", "Run!", ItemKind::Subtitle, 0.0, 1000.0), item("b", "Run!", ItemKind::Subtitle, 1200.0, 2000.0)];
        let v = judge(&items, &[ev(1300.0, "Run!"), ev(1400.0, "Run!")]);
        assert_eq!(v, vec![Verdict::Read(1), Verdict::Duplicate(1)]);

        // stack_dup: one of three stacked toasts read three times.
        let items = vec![
            item("a", "+ 1 Rope", ItemKind::PickupToast, 0.0, 3000.0),
            item("b", "+ 2 Rope", ItemKind::PickupToast, 200.0, 3000.0),
            item("c", "+ 3 Rope", ItemKind::PickupToast, 400.0, 3000.0),
        ];
        let v = judge(&items, &[ev(500.0, "+ 1 Rope"), ev(1000.0, "+ 1 Rope"), ev(1500.0, "+ 1 Rope")]);
        assert_eq!(v, vec![Verdict::Read(0), Verdict::Duplicate(0), Verdict::Duplicate(0)]);

        // noise_steals: a HUD line read twice must not count as the similar tooltip.
        let items = vec![
            item("h", "GOLD 57/120", ItemKind::HudStatic, 0.0, 10000.0),
            item("t", "Gold 5/12", ItemKind::Tooltip, 2000.0, 4000.0),
        ];
        let v = judge(&items, &[ev(100.0, "GOLD 57/120"), ev(2600.0, "GOLD 57/120")]);
        assert_eq!(v, vec![Verdict::Noise(0), Verdict::Noise(0)]);

        // Guard: two identical toasts on screen together, each read once, are both read.
        let items = vec![item("a", "+ 1 Rope", ItemKind::PickupToast, 0.0, 3000.0), item("b", "+ 1 Rope", ItemKind::PickupToast, 200.0, 3000.0)];
        let mut v = judge(&items, &[ev(500.0, "+ 1 Rope"), ev(600.0, "+ 1 Rope")]);
        v.sort_by_key(|x| format!("{x:?}"));
        assert_eq!(v, vec![Verdict::Read(0), Verdict::Read(1)]);
    }

    #[test]
    fn short_truth_needs_an_exact_read() {
        let items = vec![
            item("a", "Ash", ItemKind::Speaker, 0.0, 2000.0),
            item("b", "Nyx", ItemKind::Speaker, 0.0, 2000.0),
            item("c", "Kael", ItemKind::Speaker, 0.0, 2000.0),
            item("d", "Mara", ItemKind::Speaker, 0.0, 2000.0),
        ];
        let v = judge(&items, &[ev(100.0, "Ask"), ev(100.0, "Nyk"), ev(100.0, "Kae"), ev(100.0, "MARA:")]);
        assert_eq!(v, vec![Verdict::Hallucination, Verdict::Hallucination, Verdict::Hallucination, Verdict::Read(3)]);
        // Five chars or more keep the CER rule.
        assert_eq!(match_cer("follw", "follow"), Some(1.0 / 6.0));
        assert_eq!(match_cer("", ""), None);
    }

    #[test]
    fn punctuation_only_reads_match_nothing() {
        let items = vec![item("a", "Follow me.", ItemKind::Subtitle, 0.0, 2000.0)];
        let v = judge(&items, &[ev(100.0, "..."), ev(100.0, ""), ev(100.0, "Follow me")]);
        assert_eq!(v, vec![Verdict::Hallucination, Verdict::Hallucination, Verdict::Read(0)]);
    }

    #[test]
    fn buckets_and_latency_use_readable_time() {
        // 1500 ms on screen, but 300 ms fade-in and 400 ms fade-out: 800 ms readable, bucket <1s.
        let mut a = item("a", "faded line here", ItemKind::Subtitle, 0.0, 1500.0);
        a.anim.fade_in_ms = 300.0;
        a.anim.fade_out_ms = 400.0;
        // Typewriter: 20 chars at 10 cps, all shown 1900 ms after start; 5000 ms on screen.
        let mut b = item("b", "typed out slowly now", ItemKind::Dialogue, 2000.0, 7000.0);
        b.anim.typewriter_cps = Some(10.0);
        assert_eq!(b.readable_from_ms(), 3900.0);
        assert_eq!(crate::render::visible_chars(&b, b.readable_from_ms()), b.text.chars().count());
        assert!(crate::render::visible_chars(&b, b.readable_from_ms() - 1.0) < b.text.chars().count());
        let run = ScenarioRun {
            scenario_id: "x".into(),
            reader: "t".into(),
            clock: crate::runner::Clock::Ideal,
            events: vec![ev(100.0, "faded line here"), ev(4000.0, "typed out slowly now")],
            frame_costs_ms: vec![],
            frames_total: 0,
            frames_processed: 0,
            truth_fingerprint: String::new(),
            reader_options: Default::default(),
        };
        let items = vec![a, b];
        let m = score_scenario("x", &[], &items, &run, &BgTextMap::default()).score.metrics;
        assert_eq!(m.recall_by_duration[0].total, 1); // a: 800 ms readable
        assert_eq!(m.recall_by_duration[2].total, 1); // b: 3100 ms readable
        // a is read during its fade-in (counts as 0 ms); b 100 ms after typing ended.
        assert_eq!(m.latency_p50_ms, 0.0);
        assert_eq!(m.latency_p95_ms, 100.0);

        // Recorded items with unsure times: no latency, own bucket.
        let timing = TimingFlags::new(&["b".to_string()], &[]);
        let m = score_scenario_with("x", &[], &items, &run, &BgTextMap::default(), &timing).score.metrics;
        assert_eq!(m.recall_by_duration.len(), 5);
        assert_eq!(m.recall_by_duration[4].name, UNSURE_BUCKET);
        assert_eq!((m.recall_by_duration[4].read, m.recall_by_duration[2].total), (1, 0));
        assert_eq!(m.latency_p95_ms, 0.0);
    }

    #[test]
    fn baseline_needs_the_same_scenes() {
        let base = Report {
            suite: "standard".into(),
            seed: Some(1),
            truth_fingerprint: "abc".into(),
            scenarios: vec![ScenarioScore { id: "s-000".into(), ..Default::default() }],
            ..Default::default()
        };
        assert!(baseline_mismatch(&base, &base).is_empty());
        let other_seed = Report { seed: Some(2), truth_fingerprint: "abd".into(), ..base.clone() };
        assert_eq!(baseline_mismatch(&other_seed, &base).len(), 2);
        let old = Report { truth_fingerprint: String::new(), ..base.clone() };
        assert_eq!(baseline_mismatch(&base, &old).len(), 1);
        // Duplicates are gated too.
        let b = Metrics { duplicates: 10, ..Default::default() };
        let c = Metrics { duplicates: 13, ..Default::default() };
        assert_eq!(compare(&c, &b, Tolerance::default()).regressions.len(), 1);
    }

    #[test]
    fn percentile_nearest_rank() {
        assert_eq!(percentile(&[], 0.5), 0.0);
        assert_eq!(percentile(&[5.0, 1.0, 3.0], 0.5), 3.0);
        assert_eq!(percentile(&[1.0, 2.0, 3.0, 4.0], 0.95), 4.0);
    }
}
