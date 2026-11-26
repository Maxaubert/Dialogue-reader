//! Per-zone dedup: decides which OCR reads become spoken lines.
//!
//! Each read of a zone gives "units" (the whole zone text, or one unit per row in a notification
//! zone). Units are matched to tracks by their letters (digits ignored) and, for rows, by
//! position. So a toast that moves up a stack keeps its track, and a read with one wrong digit
//! does not start a new line. Rules:
//! - A read is "fresh" when the pixels differ from the previous read. Reading the same pixels
//!   twice gives the same text, so only fresh reads count as agreement, and only fresh reads
//!   count as misses.
//! - A track is spoken once, when (a) the same text was read `confirm` times on fresh frames, or
//!   (b) on a still screen, it stayed for `static_ms`, or (c) on a busy screen, `consensus` fresh
//!   reads agree roughly: the read closest to the others is spoken. It must also have been seen
//!   for `stable_ms` (a debounce against fade-ins, typing and flicker).
//! - Digits are voted on: the spoken digits are the ones most reads agree on.
//! - After a line was spoken, a new version is a new event when it holds for `change_ms` (with 2
//!   fresh reads, or `static_ms` on a still screen) and either a number changed
//!   ("+ 6 Iron Ore (27)" to "+ 12 Iron Ore (33)") or it has a new word ("Iron Ore" to "Iron Rod",
//!   "Stay close." to "Stay close to the wall.").
//! - Typing: when the letters grew against the newest read at least `grow_ms` old, the track
//!   waits `typing_ms` after the last growth. The rules count time, not reads, so they hold at
//!   any frame rate.
//! - A track ends after `gone_reads` misses (the first one on fresh pixels) and `gone_ms` without
//!   a read of it. The same text after that is a new event (a flicker is two events).
//! - A track that ends before it was spoken is handed back as "lost", so the caller can look at
//!   buffered frames for a better read (short pop-ups).

use crate::geom::Rect;
use crate::text::{Relation, adds_only, digits, distance, is_junk, letters, letters_key, new_words, normalize, numbers_fit, relate};

/// One unit of one OCR read.
#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    pub raw: String,
    pub norm: String,
    pub rect: Option<Rect>,
}

impl Unit {
    pub fn new(raw: &str, rect: Option<Rect>) -> Unit {
        Unit { raw: raw.to_string(), norm: normalize(raw), rect }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    pub stable_ms: f64,
    pub confirm: u32,
    pub gone_ms: f64,
    /// Misses in a row before a line can be gone (with `gone_ms`).
    pub gone_reads: u32,
    /// Largest normalised edit distance (letters only) still called the same text.
    pub jitter: f32,
    /// Fewer letters than this is junk.
    pub min_letters: usize,
    /// Fresh reads in a row that may agree only roughly (busy backgrounds make every read a bit
    /// different): after this many, speak the read closest to all the others. 0 turns it off.
    pub consensus: u32,
    /// On a still screen (no fresh reads) a read is trusted after this long.
    pub static_ms: f64,
    /// After the text last grew (typewriter), wait this long before speaking.
    pub typing_ms: f64,
    /// Typing is checked against the newest read at least this old.
    pub grow_ms: f64,
    /// A changed number or a new word must hold this long before it is a new event.
    pub change_ms: f64,
    /// A spoken line that only gains words ("Stay close." to "Stay close to the wall.") is a new
    /// event. On for dialogue; off for toasts, where a longer read of a row is mostly OCR
    /// catching the whole row.
    pub grow_events: bool,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            stable_ms: 100.0,
            confirm: 2,
            gone_ms: 100.0,
            gone_reads: 2,
            jitter: 0.3,
            min_letters: 2,
            consensus: 3,
            static_ms: 250.0,
            typing_ms: 400.0,
            grow_ms: 100.0,
            change_ms: 60.0,
            grow_events: true,
        }
    }
}

/// One distinct text seen in a track.
#[derive(Clone, Debug, PartialEq)]
pub struct Version {
    pub norm: String,
    pub raw: String,
    /// Times this exact (normalised) text was read.
    pub n: u32,
    /// Of those, reads of fresh pixels.
    pub nd: u32,
    pub first_t: f64,
    pub last_t: f64,
    pub rect: Option<Rect>,
}

/// One read that matched a track.
#[derive(Clone, Debug, PartialEq)]
pub struct Read {
    pub version: usize,
    pub t: f64,
    pub fresh: bool,
}

#[derive(Clone, Debug)]
pub struct Track {
    pub versions: Vec<Version>,
    /// Recent reads, newest last (at most `KEEP`).
    pub reads: Vec<Read>,
    /// Index of the version read last.
    pub cur: usize,
    pub first_seen: f64,
    pub last_seen: f64,
    /// Reads in a row that missed this track (the first one fresh).
    pub misses: u32,
    /// Reads in a row that matched this track.
    pub streak: u32,
    /// Normalised text and digits that were spoken last, if any.
    pub spoken: Option<(String, String)>,
    pub rect: Option<Rect>,
    /// Last time a read grew the previous one (text still being typed).
    pub grew_at: f64,
    /// A spoken track whose current read may become a new event (new number or new word).
    pub changing: bool,
    /// The spoken text was read exactly (on `confirm` fresh frames, or on a still screen), not
    /// picked from rough reads. Only then can a new word make a new event: on a busy background
    /// the reads differ by whole words all the time.
    pub solid: bool,
}

/// Reads kept per track (enough for `grow_ms` at high frame rates).
const KEEP: usize = 32;
/// Fresh reads used for the rough agreement (consensus).
const MEDOID_N: usize = 12;

impl Track {
    fn new(t: f64, u: &Unit, fresh: bool, p: &Params) -> Track {
        let mut tr = Track {
            versions: Vec::new(),
            reads: Vec::new(),
            cur: 0,
            first_seen: t,
            last_seen: t,
            misses: 0,
            streak: 0,
            spoken: None,
            rect: u.rect,
            grew_at: f64::NEG_INFINITY,
            changing: false,
            solid: false,
        };
        tr.add(t, u, fresh, p);
        tr
    }

    pub fn current(&self) -> &Version {
        &self.versions[self.cur]
    }

    /// Most read version (fresh reads first, latest on ties).
    pub fn best(&self) -> &Version {
        &self.versions[self.best_index()]
    }

    fn best_index(&self) -> usize {
        // `max_by_key` returns the last of equal items, so the latest version wins a tie.
        self.versions.iter().enumerate().max_by_key(|(_, v)| (v.nd, v.n)).map_or(0, |(i, _)| i)
    }

    /// Time of the newest fresh read that is still kept.
    pub fn last_fresh(&self) -> Option<f64> {
        self.reads.iter().rev().find(|r| r.fresh).map(|r| r.t)
    }

    fn add(&mut self, t: f64, u: &Unit, fresh: bool, p: &Params) {
        self.last_seen = t;
        if fresh {
            self.misses = 0;
        }
        self.streak += 1;
        self.rect = u.rect.or(self.rect);
        // Typing: the letters grew by 2 or more against the newest read at least `grow_ms` old
        // (the oldest read while the track is younger), and that read is the start of this one,
        // up to OCR noise. For slow typing (one letter per 100 ms) the newest read at least 2.5
        // times as old is checked too, with an exact start only (noisy reads of a busy screen
        // often look like a start). Junk at the end adds no letters, and OCR flicker of one
        // letter ("Clim" and "Climb") never looks like typing.
        let ln = letters(&u.norm);
        let grew = |r: Option<&Read>, fuzzy: bool| {
            r.is_some_and(|r| {
                let pn = &self.versions[r.version].norm;
                let start = if fuzzy { crate::text::is_growing_prefix(pn, &u.norm) } else { pn.chars().count() >= 3 && u.norm.starts_with(pn.as_str()) };
                ln >= letters(pn) + 2 && (start || (pn.chars().count() >= 2 && u.norm.starts_with(pn.as_str())))
            })
        };
        let near = self.reads.iter().rev().find(|r| t - r.t >= p.grow_ms - 1e-6).or(self.reads.first());
        let far = self.reads.iter().rev().find(|r| t - r.t >= p.grow_ms * 2.5 - 1e-6);
        if grew(near, true) || grew(far, false) {
            // The growth happened when this text first showed up.
            let since = match self.versions.iter().position(|v| v.norm == u.norm) {
                Some(i) if i == self.cur => self.versions[i].first_t,
                _ => t,
            };
            self.grew_at = self.grew_at.max(since);
        }
        let i = match self.versions.iter().position(|v| v.norm == u.norm) {
            Some(i) => {
                let v = &mut self.versions[i];
                v.n += 1;
                v.nd += u32::from(fresh);
                v.last_t = t;
                v.rect = u.rect.or(v.rect);
                // A version that comes back after another one starts its time again.
                if self.cur != i {
                    v.first_t = t;
                }
                i
            }
            None => {
                self.versions.push(Version { norm: u.norm.clone(), raw: u.raw.clone(), n: 1, nd: u32::from(fresh), first_t: t, last_t: t, rect: u.rect });
                self.versions.len() - 1
            }
        };
        self.cur = i;
        self.reads.push(Read { version: i, t, fresh });
        if self.reads.len() > KEEP {
            self.reads.remove(0);
        }
    }

    /// Reads of the current streak, oldest first.
    fn streak_reads(&self) -> &[Read] {
        &self.reads[self.reads.len().saturating_sub(self.streak as usize)..]
    }

    /// Fresh reads of the current streak (at most the last `n`), as version indices.
    fn fresh_reads(&self, n: usize) -> Vec<usize> {
        let v: Vec<usize> = self.streak_reads().iter().filter(|r| r.fresh).map(|r| r.version).collect();
        v[v.len().saturating_sub(n)..].to_vec()
    }

    /// Of the reads `ids`, the one closest to the others, with its mean distance to them.
    pub fn medoid_of(&self, ids: &[usize]) -> Option<(usize, f32)> {
        if ids.len() < 2 {
            return None;
        }
        // Distances between distinct versions, worked out once.
        let mut uniq: Vec<usize> = ids.to_vec();
        uniq.sort_unstable();
        uniq.dedup();
        let k = uniq.len();
        let pos = |v: usize| uniq.binary_search(&v).unwrap_or(0);
        let mut dm = vec![0f32; k * k];
        for a in 0..k {
            for b in a + 1..k {
                let d = distance(&self.versions[uniq[a]].norm, &self.versions[uniq[b]].norm);
                dm[a * k + b] = d;
                dm[b * k + a] = d;
            }
        }
        let mut best: Option<(usize, f32)> = None;
        for &a in ids {
            let pa = pos(a);
            let d: f32 = ids.iter().map(|&b| dm[pa * k + pos(b)]).sum::<f32>() / (ids.len() - 1) as f32;
            // Prefer the later read on ties (more faded in).
            if best.is_none_or(|(_, bd)| d <= bd) {
                best = Some((a, d));
            }
        }
        best
    }

    /// The numbers most fresh reads of the streak support, with that support. A read supports a
    /// value when its numbers fit inside it (a read that missed a number still supports the full
    /// value). Ties go to the longer value.
    fn digit_vote(&self) -> (String, usize) {
        let vals: Vec<String> = self.streak_reads().iter().filter(|r| r.fresh).map(|r| digits(&self.versions[r.version].norm)).collect();
        let mut best = (String::new(), 0usize);
        for v in &vals {
            let support = vals.iter().filter(|o| numbers_fit(o, v) && o.len() <= v.len()).count();
            if support > best.1 || (support == best.1 && v.len() > best.0.len()) {
                best = (v.clone(), support);
            }
        }
        best
    }

    /// The trailing run of reads whose numbers fit `d` but not `old`: (reads, fresh reads, first
    /// time). Reads that fit both are skipped; a read that fits only `old` ends the run.
    fn digit_run(&self, d: &str, old: &str) -> (usize, usize, f64) {
        let mut n = 0;
        let mut nf = 0;
        let mut first = f64::INFINITY;
        for r in self.streak_reads().iter().rev() {
            let v = digits(&self.versions[r.version].norm);
            let (fits_new, fits_old) = (numbers_fit(&v, d), numbers_fit(&v, old));
            if fits_new && fits_old {
                continue;
            }
            if !fits_new {
                break;
            }
            n += 1;
            nf += usize::from(r.fresh);
            first = r.t;
        }
        (n, nf, first)
    }

    /// The trailing run of reads of exactly `cand`: (reads, fresh reads, first time).
    fn exact_run(&self, cand: usize) -> (usize, usize, f64) {
        let mut n = 0;
        let mut nf = 0;
        let mut first = f64::INFINITY;
        for r in self.streak_reads().iter().rev() {
            if r.version != cand {
                break;
            }
            n += 1;
            nf += usize::from(r.fresh);
            first = r.t;
        }
        (n, nf, first)
    }

    /// Picks the version to speak now, if the track is ready.
    fn ready(&self, t: f64, p: &Params) -> Option<usize> {
        if t - self.grew_at < p.typing_ms - 1e-6 {
            return None;
        }
        let v = self.current();
        let seen = t - v.first_t;
        // A text of one or two letters may be the start of a typed line ("Go" of "Go to the
        // lighthouse"): it waits the typing time, so the next letters can show up.
        let stable = if letters(&v.norm) < 3 { p.stable_ms.max(p.typing_ms) } else { p.stable_ms };
        let exact = v.n >= p.confirm && self.streak >= 2.min(p.confirm) && seen >= stable - 1e-6 && (v.nd >= p.confirm || seen >= p.static_ms - 1e-6);
        let mut pick = exact.then_some(self.cur);
        if pick.is_none() && p.consensus >= 2 && t - self.first_seen >= stable - 1e-6 {
            let ids = self.fresh_reads(MEDOID_N);
            if ids.len() >= p.consensus as usize
                && let Some((m, d)) = self.medoid_of(&ids)
                && d <= p.jitter
                // Not the exact start of the text read now (a line still being typed).
                && !(letters(&v.norm) > letters(&self.versions[m].norm) && v.norm.starts_with(self.versions[m].norm.as_str()))
                && (letters(&self.versions[m].norm) >= 3 || t - self.first_seen >= p.stable_ms.max(p.typing_ms) - 1e-6)
            {
                pick = Some(m);
            }
        }
        let pick = pick?;
        // Digit vote: speak the number most reads agree on.
        let (dv, c) = self.digit_vote();
        if c >= 2 && digits(&self.versions[pick].norm) != dv {
            let key = letters_key(&self.versions[pick].norm);
            let alt = self
                .versions
                .iter()
                .enumerate()
                .filter(|(_, v)| digits(&v.norm) == dv)
                .min_by(|a, b| distance(&letters_key(&a.1.norm), &key).total_cmp(&distance(&letters_key(&b.1.norm), &key)))
                .map(|(i, _)| i);
            return alt.or(Some(pick));
        }
        Some(pick)
    }

    /// A spoken track: is the current read a new event? Returns the version to speak.
    fn changed_event(&mut self, t: f64, p: &Params) -> Option<Version> {
        let (s, sd) = self.spoken.clone()?;
        let v = self.current().clone();
        let d = digits(&v.norm);
        self.changing = false;
        // A number that changed and held is a new event.
        if !numbers_fit(&d, &sd) {
            self.changing = true;
            let (n, nf, first) = self.digit_run(&d, &sd);
            let held = t - first;
            if n >= 2 && held >= p.change_ms - 1e-6 && (nf >= 2 || held >= p.static_ms - 1e-6) {
                // Speak the fullest read of the new number (most numbers, then most reads).
                let w = self
                    .versions
                    .iter()
                    .filter(|x| x.last_t >= first && numbers_fit(&digits(&x.norm), &d) && !numbers_fit(&digits(&x.norm), &sd))
                    .max_by_key(|x| (digits(&x.norm).len(), x.n))
                    .cloned()
                    .unwrap_or(v);
                self.changing = false;
                return Some(w);
            }
            return None;
        }
        // A new word that held (and is no longer being typed) is a new line. Every read since it
        // appeared must be exactly that text: OCR noise does not repeat itself on fresh pixels.
        if self.solid && v.norm != s && new_words(&s, &v.norm) && (p.grow_events || !adds_only(&s, &v.norm)) && !is_junk(&v.raw, p.min_letters) {
            self.changing = true;
            if t - self.grew_at < p.typing_ms - 1e-6 {
                return None;
            }
            // The new text must be new against every text read before it (OCR flipping between
            // "Steel Plate" and "Steet Plate" is one line read two ways).
            let (n, nf, first) = self.exact_run(self.cur);
            let held = t - first;
            let fresh_text = v.n as usize == n && self.versions.iter().filter(|w| w.last_t < first).all(|w| new_words(&w.norm, &v.norm));
            if n >= 2 && fresh_text && held >= p.change_ms.max(p.stable_ms) - 1e-6 && (nf >= p.confirm as usize || held >= p.static_ms - 1e-6) {
                self.changing = false;
                return Some(v);
            }
        }
        None
    }
}

/// A line to speak now.
#[derive(Clone, Debug, PartialEq)]
pub struct Speak {
    pub raw: String,
    pub rect: Option<Rect>,
}

/// A track that ended without being spoken.
#[derive(Clone, Debug)]
pub struct Lost {
    pub track: Track,
}

impl Lost {
    /// The text to speak for a line that ended early: the version read on `confirm` fresh frames,
    /// or a close consensus of fresh reads (half the usual jitter, as no later read checks it).
    /// None when the reads do not agree.
    pub fn agreed(&self, p: &Params) -> Option<&Version> {
        let tr = &self.track;
        // Still being typed when it ended: a later read took over, so this is a part of a line.
        if tr.last_seen - tr.grew_at < p.typing_ms {
            return None;
        }
        let b = tr.best();
        if b.nd >= p.confirm {
            return Some(b);
        }
        if p.consensus >= 2 {
            let ids: Vec<usize> = tr.reads.iter().filter(|r| r.fresh).map(|r| r.version).collect();
            let ids = &ids[ids.len().saturating_sub(MEDOID_N)..];
            if ids.len() >= p.consensus as usize
                && let Some((m, d)) = tr.medoid_of(ids)
                && d <= p.jitter * 0.5
            {
                return Some(&tr.versions[m]);
            }
        }
        None
    }
}

#[derive(Default)]
pub struct Tracker {
    pub params: Params,
    pub tracks: Vec<Track>,
}

impl Tracker {
    pub fn new(params: Params) -> Tracker {
        Tracker { params, tracks: Vec::new() }
    }

    /// True while some track still waits to be spoken, was missed in the last fresh read (so a
    /// second read can tell whether it is gone), or may be changing into a new event.
    pub fn pending(&self) -> bool {
        self.tracks.iter().any(|t| t.spoken.is_none() || t.misses > 0 || t.changing)
    }

    pub fn has_tracks(&self) -> bool {
        !self.tracks.is_empty()
    }

    /// The pixels did not change since the last read: the lines read then are still on screen.
    pub fn touch(&mut self, t: f64) {
        for tr in &mut self.tracks {
            if tr.streak > 0 {
                tr.last_seen = t;
            }
        }
    }

    /// Feeds one OCR read of the zone at time `t`. `fresh` is false when the pixels did not change
    /// since the previous read.
    pub fn update(&mut self, t: f64, units: &[Unit], fresh: bool) -> (Vec<Speak>, Vec<Lost>) {
        let p = self.params.clone();
        let units: Vec<&Unit> = units.iter().filter(|u| !is_junk(&u.raw, p.min_letters)).collect();
        // Versions to match against per track (current, best and rough agreement), with their
        // letter keys, worked out once per read.
        let anchors: Vec<Vec<(usize, String)>> = self
            .tracks
            .iter()
            .map(|tr| {
                let first_best = tr.versions.iter().enumerate().rev().max_by_key(|(_, v)| (v.nd, v.n)).map_or(0, |(i, _)| i);
                let mut a = vec![tr.cur, first_best];
                if let Some((m, _)) = tr.medoid_of(&tr.fresh_reads(8)) {
                    a.push(m);
                }
                a.sort_unstable();
                a.dedup();
                a.into_iter().map(|i| (i, letters_key(&tr.versions[i].norm))).collect()
            })
            .collect();
        // Candidate pairs (cost, unit, track).
        let mut pairs = Vec::new();
        for (ui, u) in units.iter().enumerate() {
            let uk = letters_key(&u.norm);
            for (ti, tr) in self.tracks.iter().enumerate() {
                let mut best: Option<f32> = None;
                for (vi, vk) in &anchors[ti] {
                    let c = match relate(&u.norm, &tr.versions[*vi].norm, p.jitter) {
                        Relation::Same => Some(distance(&uk, vk)),
                        Relation::NumberChanged => Some(0.5 + distance(&uk, vk)),
                        Relation::Grows => Some(1.0),
                        Relation::Different => None,
                    };
                    if let Some(c) = c {
                        best = Some(best.map_or(c, |b: f32| b.min(c)));
                    }
                }
                if let (Some(c), Some(a), Some(b)) = (best, u.rect, tr.rect) {
                    // Rows: prefer the track at the same height.
                    let dy = ((a.y + a.h / 2.0) - (b.y + b.h / 2.0)).abs() / a.h.max(b.h).max(1.0);
                    best = Some(c + 0.5 * dy.min(2.0));
                }
                if let Some(c) = best {
                    pairs.push((c, ui, ti));
                }
            }
        }
        pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut unit_used = vec![false; units.len()];
        let mut track_used = vec![false; self.tracks.len()];
        for (_, ui, ti) in pairs {
            if unit_used[ui] || track_used[ti] {
                continue;
            }
            unit_used[ui] = true;
            track_used[ti] = true;
            self.tracks[ti].add(t, units[ui], fresh, &p);
        }
        let new_tracks: Vec<Track> = units.iter().enumerate().filter(|(ui, _)| !unit_used[*ui]).map(|(_, u)| Track::new(t, u, fresh, &p)).collect();
        // Misses and ends. Only a fresh read can start a miss: the same pixels read again tell
        // nothing new. After that, every read without the line counts, and the line ends when
        // `gone_ms` passed since it was last seen (`touch` keeps that time current on a still
        // screen, so a short glitch does not end a line).
        let mut lost = Vec::new();
        let mut keep = Vec::new();
        for (ti, mut tr) in std::mem::take(&mut self.tracks).into_iter().enumerate() {
            if !track_used[ti] {
                tr.misses += u32::from(fresh || tr.misses > 0);
                tr.streak = 0;
            }
            let away = t - tr.last_seen;
            let gone = tr.misses >= p.gone_reads && away >= p.gone_ms - 1e-6;
            if gone {
                if tr.spoken.is_none() {
                    lost.push(Lost { track: tr });
                }
            } else {
                keep.push(tr);
            }
        }
        keep.extend(new_tracks);
        self.tracks = keep;
        // Speak what is ready.
        let mut out = Vec::new();
        for tr in &mut self.tracks {
            if tr.streak == 0 {
                continue;
            }
            if tr.spoken.is_none() {
                if let Some(i) = tr.ready(t, &p) {
                    let v = &tr.versions[i];
                    out.push(Speak { raw: v.raw.clone(), rect: v.rect });
                    tr.spoken = Some((v.norm.clone(), digits(&v.norm)));
                    tr.solid = i == tr.cur && (v.nd >= p.confirm || t - v.first_t >= p.static_ms - 1e-6);
                }
            } else if let Some(w) = tr.changed_event(t, &p) {
                out.push(Speak { raw: w.raw.clone(), rect: w.rect });
                tr.spoken = Some((w.norm.clone(), digits(&w.norm)));
                tr.solid = tr.current().norm == w.norm;
            }
        }
        (out, lost)
    }

    /// Ends every track (scene end or zone reset); returns the ones never spoken.
    pub fn drain(&mut self) -> Vec<Lost> {
        std::mem::take(&mut self.tracks).into_iter().filter(|t| t.spoken.is_none()).map(|track| Lost { track }).collect()
    }

    /// Is a line like `norm` on screen in this zone (spoken or waiting)?
    pub fn knows(&self, norm: &str) -> bool {
        self.tracks.iter().any(|tr| tr.versions.iter().any(|v| relate(norm, &v.norm, self.params.jitter) == Relation::Same))
    }

    /// Does a live track hold a longer text that starts with `norm` (so `norm` is a part of it)?
    pub fn grows_from(&self, norm: &str) -> bool {
        self.tracks.iter().any(|tr| tr.versions.iter().any(|v| crate::text::is_growing_prefix(norm, &v.norm)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Unit {
        Unit::new(s, None)
    }

    fn texts(v: &[Speak]) -> Vec<String> {
        v.iter().map(|s| s.raw.clone()).collect()
    }

    fn params() -> Params {
        Params { stable_ms: 60.0, gone_ms: 60.0, consensus: 0, ..Params::default() }
    }

    /// Feeds fresh reads 33 ms apart; returns everything spoken.
    fn feed(tr: &mut Tracker, reads: &[&[&str]]) -> Vec<String> {
        feed_with(tr, reads, true)
    }

    fn feed_with(tr: &mut Tracker, reads: &[&[&str]], fresh: bool) -> Vec<String> {
        let mut out = Vec::new();
        for (i, r) in reads.iter().enumerate() {
            let units: Vec<Unit> = r.iter().map(|s| u(s)).collect();
            out.extend(texts(&tr.update(i as f64 * 33.0, &units, fresh || i == 0).0));
        }
        out
    }

    #[test]
    fn speaks_once_while_the_line_stays() {
        let mut tr = Tracker::new(params());
        let said = feed(&mut tr, &[&["Hello there"], &["Hello there"], &["Hello there"], &["Hel1o there"], &["Hello there"], &["Hello there"]]);
        assert_eq!(said, vec!["Hello there"]);
    }

    #[test]
    fn same_pixels_read_twice_do_not_confirm() {
        // Reads of an unchanged frame agree, but that is no evidence: wait for static_ms.
        let mut tr = Tracker::new(params());
        let said = feed_with(&mut tr, &[&["Ostrava"], &["Ostrava"], &["Ostrava"]], false);
        assert!(said.is_empty());
        let mut tr = Tracker::new(params());
        let reads: Vec<&[&str]> = vec![&["Ostrava"]; 10];
        let said = feed_with(&mut tr, &reads, false);
        assert_eq!(said, vec!["Ostrava"]);
    }

    #[test]
    fn same_text_after_it_left_is_a_new_event() {
        let mut tr = Tracker::new(params());
        let said = feed(&mut tr, &[&["Kael"], &["Kael"], &["Kael"], &[], &[], &[], &["Kael"], &["Kael"], &["Kael"]]);
        assert_eq!(said, vec!["Kael", "Kael"]);
    }

    #[test]
    fn one_missed_read_is_not_a_new_event() {
        let mut tr = Tracker::new(params());
        let said = feed(&mut tr, &[&["Main Quest: Find Ada"], &["Main Quest: Find Ada"], &["Main Quest: Find Ada"], &[], &["Main Quest: Find Ada"], &["Main Quest: Find Ada"]]);
        assert_eq!(said.len(), 1);
    }

    #[test]
    fn changed_number_is_a_new_line() {
        let mut tr = Tracker::new(params());
        let a = "+ 6 Iron Ore (27)";
        let b = "+ 12 Iron Ore (33)";
        let said = feed(&mut tr, &[&[a], &[a], &[a], &[b], &[b], &[b], &[b]]);
        assert_eq!(said, vec![a, b]);
    }

    #[test]
    fn digit_misreads_are_voted_out() {
        let mut tr = Tracker::new(params());
        let a = "+ 6 Iron Ore (27)";
        let said = feed(&mut tr, &[&[a], &[a], &[a], &["+ 6 Iron Ore (21)"], &[a], &["46 Iron Ore (27)"], &[a], &[a]]);
        assert_eq!(said, vec![a]);
        assert_eq!(tr.tracks.len(), 1);
    }

    #[test]
    fn typewriter_line_is_spoken_when_done() {
        let mut tr = Tracker::new(Params { typing_ms: 0.0, ..params() });
        let said = feed(
            &mut tr,
            &[&["Go to"], &["Go to the li"], &["Go to the lighth"], &["Go to the lighthouse."], &["Go to the lighthouse."], &["Go to the lighthouse."]],
        );
        assert_eq!(said, vec!["Go to the lighthouse."]);
    }

    #[test]
    fn slow_typing_waits_for_the_end() {
        // The text pauses between words for longer than stable_ms; it must not be spoken early.
        let mut tr = Tracker::new(Params { typing_ms: 300.0, ..params() });
        let mut reads: Vec<&[&str]> = vec![&["Ev"], &["Every"]];
        reads.extend([&["Every name"][..]; 4]);
        reads.extend([&["Every name on this"][..]; 4]);
        reads.extend([&["Every name on this wall is someone we lost."][..]; 16]);
        reads.extend([&["Every name on this wall is someone we lost. 2"][..], &["Every name on this wall is someone we lost."][..]]);
        let said = feed(&mut tr, &reads);
        assert_eq!(said, vec!["Every name on this wall is someone we lost."]);
    }

    #[test]
    fn stacked_toasts_one_event_each_and_remembered_when_the_stack_moves() {
        let mut tr = Tracker::new(params());
        let said = feed(
            &mut tr,
            &[
                &["+ 9 Leaves (9)"],
                &["+ 9 Leaves (9)"],
                &["+ 9 Leaves (9)", "+ 1 Wood (1)"],
                &["+ 1 Wood (1)", "+ 9 Leaves (9)"],
                &["+ 1 Wood (1)", "+ 9 Leaves (9)"],
                &["+ 1 Wood (1)", "+ 9 Leaves (9)"],
            ],
        );
        assert_eq!(said, vec!["+ 9 Leaves (9)", "+ 1 Wood (1)"]);
    }

    #[test]
    fn junk_is_never_spoken_and_short_reads_must_agree_exactly() {
        let mut tr = Tracker::new(params());
        let said = feed(&mut tr, &[&["-", "I"], &["-", "I"], &["~~"], &["Nyx"], &["Nyr"], &["Nyx"], &["Nyr"]]);
        assert!(said.is_empty(), "{said:?}");
    }

    #[test]
    fn noisy_reads_reach_a_consensus() {
        let mut tr = Tracker::new(Params { consensus: 3, ..params() });
        let said = feed(
            &mut tr,
            &[&["The old road is saZfer, but"], &["The old roadi is safer, but"], &["The old road is safer, bvt"], &["The old r0ad is safer, but"]],
        );
        assert_eq!(said.len(), 1);
        // Without consensus no two reads agree, so nothing is spoken.
        let mut tr = Tracker::new(params());
        let said = feed(&mut tr, &[&["The old road is saZfer, but"], &["The old roadi is safer, but"], &["The old road is safer, bvt"]]);
        assert!(said.is_empty());
    }

    #[test]
    fn lost_tracks_are_handed_back_with_their_agreement() {
        let mut tr = Tracker::new(Params { stable_ms: 500.0, ..params() });
        tr.update(0.0, &[u("Quick toast")], true);
        tr.update(33.0, &[u("Quick toast")], true);
        tr.update(66.0, &[], true);
        let (_, lost) = tr.update(100.0, &[], true);
        assert_eq!(lost.len(), 1);
        assert_eq!(lost[0].agreed(&tr.params).map(|v| v.raw.as_str()), Some("Quick toast"));
        assert!(tr.drain().is_empty());
        // One read only: no agreement.
        tr.update(200.0, &[u("Other toast")], true);
        let lost = tr.drain();
        assert!(lost[0].agreed(&tr.params).is_none());
    }

    #[test]
    fn a_line_that_grows_after_it_was_spoken_is_spoken_again() {
        let mut tr = Tracker::new(Params { typing_ms: 0.0, ..params() });
        let said = feed(&mut tr, &[&["Stay close"], &["Stay close"], &["Stay close"], &["Stay close to the wall"], &["Stay close to the wall"], &["Stay close to the wall"]]);
        assert_eq!(said, vec!["Stay close", "Stay close to the wall"]);
    }
}
