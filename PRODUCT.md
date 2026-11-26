# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

(The shipped app is a native Windows desktop app built with Slint. Mockups are HTML that must stay inside what Slint can draw: flat fills, borders, rounded rects, simple shadows, no blur/backdrop filters or CSS-only tricks.)

## Stack

Rust workspace. Engine `dr-core`, UI in Slint (chosen 2026-11-22 over Tauri, egui and Qt). Mockups: static HTML/CSS in `design/mockups/`.

## Users

Max first: a PC gamer who wants on-screen game text (dialogue, item pickups, prompts, tooltips) read aloud while playing. Public release later, so the UI must explain itself to a stranger without a manual.

## Product Purpose

DialogReader watches a game window, finds text in user-drawn zones with OCR, and speaks it with local TTS. Success: every line worth hearing is spoken once, fast, including pop-ups that last under a second, with no junk.

## Positioning

Per-game zones with types (dialogue, speaker, notification, prompt, ignore) plus a frame ring that rescues short pop-ups. Two modes: automatic, or manual where one key press reads all zones.

## Operating Context

- Setup happens before play, in the main window: pick the game window, draw zones over a screenshot, press the read key to bind it, pick and test a voice, pick auto or manual.
- During play the main window is closed. The app lives in the tray plus a tiny in-game overlay showing status and the last line read.
- Each game has its own profile (zones, mode, key, voice).
- Games run borderless or windowed, often 4K.

## Capabilities and Constraints

- Zone kinds: dialogue, speaker, notification, prompt, ignore. Prompts can be switched off.
- Modes: auto (continuous) and manual (key press reads all zones, nothing outside zones).
- The read key is chosen by the user in onboarding; there is no fixed default.
- Live log of read lines with replay.
- Undecided: voice list and engines (Kokoro planned), overlay position options.

## Brand Commitments

Name: DialogReader. The old UI was ugly and unintuitive; it is an anti-reference only. Nothing from it is kept.

## Evidence on Hand

Arena benchmark numbers in `docs/arena-baseline.md` and `docs/reader-a1.md`. Satisfactory recording (local only). No users, testimonials or press exist; never fabricate them.

## Product Principles

1. Setup in minutes, then forget it while playing.
2. Never steal focus or get in the way of the game.
3. Show what was read and why, so a miss or junk read is easy to fix.
4. Plain words, no jargon (OCR, CER) in the UI.

## Accessibility & Inclusion

Players who rely on speech for reading; keep text large enough and contrast high, and make every action keyboard reachable.
