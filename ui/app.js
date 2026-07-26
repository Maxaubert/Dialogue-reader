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
