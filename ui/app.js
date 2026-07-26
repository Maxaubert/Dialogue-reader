/* Settings page logic. Talks to ui/api.py through window.pywebview.api. */

let API = null;
let initialHotkeys = {};
let initialNoSpeakerVoice = null;

function apiReady() {
  return new Promise((resolve) => {
    if (window.pywebview && window.pywebview.api) return resolve();
    window.addEventListener("pywebviewready", resolve);
  });
}

const $ = (sel) => document.querySelector(sel);
const fields = () => document.querySelectorAll("[data-s]");

function voiceLabel(v) {
  // kokoro:af_heart -> "af heart"
  return v.replace(/^kokoro:/, "").replace(/_/g, " ");
}

function markDirty() {
  $("#savebar").classList.remove("hidden");
}

function buildVoiceGrid(voices, pool) {
  const grid = $("#voice-grid");
  grid.replaceChildren();
  const useAll = pool.trim() === "kokoro:all";
  $("#use-all-voices").checked = useAll;
  const selected = new Set(
    useAll ? voices : pool.split(",").map((s) => s.trim()).filter(Boolean)
  );
  for (const v of voices) {
    const label = document.createElement("label");
    const cb = document.createElement("input");
    cb.type = "checkbox";
    cb.className = "voice-cb";
    cb.value = v;
    cb.checked = selected.has(v);
    cb.disabled = useAll;
    cb.addEventListener("change", markDirty);
    const play = document.createElement("button");
    play.textContent = "▶";
    play.className = "mini";
    play.title = "Preview " + voiceLabel(v);
    play.addEventListener("click", (e) => {
      e.preventDefault();
      API.preview_voice(v);
    });
    label.append(cb, document.createTextNode(voiceLabel(v)), play);
    grid.append(label);
  }
  grid.classList.toggle("dim", useAll);
}

function buildDefaultVoice(voices, current) {
  const sel = $("#default-voice");
  sel.replaceChildren();
  for (const v of voices) {
    const o = document.createElement("option");
    o.value = v;
    o.textContent = voiceLabel(v);
    sel.append(o);
  }
  sel.value = voices.includes(current) ? current : voices[0];
}

function buildHotkeys(hotkeys) {
  const box = $("#hotkeys");
  box.replaceChildren();
  for (const [key, val] of Object.entries(hotkeys)) {
    const label = document.createElement("label");
    const input = document.createElement("input");
    input.type = "text";
    input.value = val;
    input.dataset.s = "Hotkeys";
    input.dataset.k = key;
    input.addEventListener("input", () => {
      markDirty();
      $("#restart-hint").classList.remove("hidden");
    });
    label.append(document.createTextNode(key.replace(/([A-Z])/g, " $1").trim()), input);
    box.append(label);
  }
  initialHotkeys = { ...hotkeys };
}

function fillFields(settings) {
  for (const el of fields()) {
    const val = settings[el.dataset.s]?.[el.dataset.k];
    if (val === undefined) continue;
    if (el.type === "checkbox") el.checked = !!val;
    else el.value = val;
  }
}

function collect() {
  const out = {};
  for (const el of fields()) {
    const s = el.dataset.s, k = el.dataset.k;
    out[s] = out[s] || {};
    if (el.type === "checkbox") out[s][k] = el.checked;
    else if (el.type === "number") out[s][k] = parseInt(el.value || "0", 10);
    else out[s][k] = el.value;
  }
  // Voice pool from the grid
  const useAll = $("#use-all-voices").checked;
  const checked = [...document.querySelectorAll(".voice-cb:checked")].map((c) => c.value);
  out.Voices = out.Voices || {};
  out.Voices.Pool = useAll || checked.length === 0 ? "kokoro:all" : checked.join(",");
  return out;
}

const SVG_NS = "http://www.w3.org/2000/svg";
const MODE_FILL = { dialogue: "rgba(120,220,140,0.35)", speaker: "rgba(255,170,60,0.4)" };
const MODE_STROKE = { dialogue: "#78dc8c", speaker: "#ffaa3c" };

function profilePreview(p) {
  // Miniature of the game window with the profile's boxes, to scale.
  const w = p.window?.w || 1920, h = p.window?.h || 1080;
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", `0 0 ${w} ${h}`);
  svg.setAttribute("width", "150");
  svg.setAttribute("height", Math.max(40, Math.round((150 * h) / w)));
  for (const r of p.regions || []) {
    const rect = document.createElementNS(SVG_NS, "rect");
    rect.setAttribute("x", r.rel_x);
    rect.setAttribute("y", r.rel_y);
    rect.setAttribute("width", r.w);
    rect.setAttribute("height", r.h);
    rect.setAttribute("fill", MODE_FILL[r.mode] || MODE_FILL.dialogue);
    rect.setAttribute("stroke", MODE_STROKE[r.mode] || MODE_STROKE.dialogue);
    rect.setAttribute("stroke-width", Math.max(2, Math.round(w / 300)));
    if (r.rotation) {
      rect.setAttribute("transform",
        `rotate(${r.rotation} ${r.rel_x + r.w / 2} ${r.rel_y + r.h / 2})`);
    }
    svg.append(rect);
  }
  return svg;
}

function buildProfiles(profiles) {
  const list = $("#profile-list");
  list.replaceChildren();
  const names = Object.keys(profiles || {});
  if (!names.length) {
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = "No profiles yet. Set up boxes in a game (F1), then save them here.";
    list.append(empty);
    return;
  }
  for (const name of names) {
    const p = profiles[name];
    const row = document.createElement("div");
    row.className = "profile";
    row.append(profilePreview(p));

    const info = document.createElement("div");
    info.className = "p-info";
    const title = document.createElement("div");
    title.className = "p-name";
    title.textContent = name;
    const meta = document.createElement("div");
    meta.className = "p-meta";
    meta.textContent =
      `${p.process} · ${(p.regions || []).length} box(es) · ${p.window?.w}×${p.window?.h}`;
    info.append(title, meta);
    row.append(info);

    const controls = document.createElement("div");
    controls.className = "p-controls";

    const applied = document.createElement("label");
    const appliedCb = document.createElement("input");
    appliedCb.type = "checkbox";
    appliedCb.className = "p-applied";
    appliedCb.checked = !!p.applied;
    appliedCb.addEventListener("change", async () => {
      if (appliedCb.checked) await API.profile_apply(name);
      else await API.profile_unapply(name);
      scheduleProfileRefresh();
    });
    applied.append(appliedCb, document.createTextNode("Applied"));

    const auto = document.createElement("label");
    const autoCb = document.createElement("input");
    autoCb.type = "checkbox";
    autoCb.className = "p-auto";
    autoCb.checked = !!p.apply_on_launch;
    autoCb.addEventListener("change", async () => {
      await API.profile_auto(name, autoCb.checked);
      scheduleProfileRefresh();
    });
    auto.append(autoCb, document.createTextNode("Auto"));

    const del = document.createElement("button");
    del.textContent = "🗑";
    del.className = "mini";
    del.title = "Delete profile";
    del.addEventListener("click", async () => {
      await API.profile_delete(name);
      scheduleProfileRefresh();
    });

    controls.append(applied, auto, del);
    row.append(controls);
    list.append(row);
  }
}

let profileRefreshTimer = null;
function scheduleProfileRefresh() {
  // The reader needs a beat to process the command and rewrite profiles.json.
  clearTimeout(profileRefreshTimer);
  profileRefreshTimer = setTimeout(refreshProfiles, 1200);
}

async function refreshProfiles() {
  buildProfiles(await API.get_profiles());
}

async function refreshStatus() {
  const running = await API.reader_status();
  const pill = $("#status");
  pill.textContent = running ? "reader running" : "reader not running";
  pill.classList.toggle("on", running);
  pill.classList.toggle("off", !running);
}

async function init() {
  await apiReady();
  API = window.pywebview.api;
  const state = await API.get_state();

  fillFields(state.settings);
  buildProfiles(state.profiles);
  setInterval(refreshProfiles, 5000);   // auto-apply changes state in the bg
  $("#btn-profile-save").addEventListener("click", async () => {
    const name = $("#profile-name").value.trim();
    if (!name) return;
    await API.profile_save(name);
    $("#profile-name").value = "";
    scheduleProfileRefresh();
  });
  // The select shows the LIVE no-speaker voice (speakers.json __default__),
  // which wins over the ini Voices.Default once F2 has ever been pressed.
  initialNoSpeakerVoice = state.no_speaker_voice;
  buildDefaultVoice(state.voices, state.no_speaker_voice);
  buildVoiceGrid(state.voices, state.settings.Voices.Pool);
  buildHotkeys(state.settings.Hotkeys);

  const pill = $("#status");
  pill.textContent = state.running ? "reader running" : "reader not running";
  pill.classList.toggle("on", state.running);
  pill.classList.toggle("off", !state.running);
  setInterval(refreshStatus, 5000);

  for (const el of fields()) el.addEventListener("change", markDirty);
  $("#use-all-voices").addEventListener("change", () => {
    const all = $("#use-all-voices").checked;
    for (const cb of document.querySelectorAll(".voice-cb")) cb.disabled = all;
    markDirty();
  });

  $("#btn-save").addEventListener("click", async () => {
    await API.save_settings(collect());
    const nsv = $("#default-voice").value;
    if (nsv !== initialNoSpeakerVoice) {
      await API.set_no_speaker_voice(nsv);
      initialNoSpeakerVoice = nsv;
    }
    $("#savebar").classList.add("hidden");
    const hk = collect().Hotkeys || {};
    const changed = Object.keys(hk).some((k) => hk[k] !== initialHotkeys[k]);
    if (changed) $("#restart-hint").classList.remove("hidden");
  });

  $("#btn-pause").addEventListener("click", () => API.live_command("TOGGLE_PAUSE"));
  $("#btn-slower").addEventListener("click", () => API.live_command("SPEED_DOWN"));
  $("#btn-faster").addEventListener("click", () => API.live_command("SPEED_UP"));
  $("#btn-cycle").addEventListener("click", () => API.live_command("CYCLE_VOICE"));
  $("#preview-default").addEventListener("click", (e) => {
    e.preventDefault();
    API.preview_voice($("#default-voice").value);
  });
  $("#btn-restart").addEventListener("click", async () => {
    await API.restart_reader();
    $("#restart-hint").classList.add("hidden");
  });
}

init();
