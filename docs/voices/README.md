# Voice Catalogs

Reference CSVs for every voice the dialogue reader knows about. Pick the ones
you want and list them in `dialogue_reader.ini`'s `[Voices]` `Pool=` line.

| Engine / model          | Catalog file            | Count | Shorthand support      |
|-------------------------|-------------------------|-------|------------------------|
| Piper (curated)         | `piper.csv`             | 12    | `piper:all`            |
| Kokoro-82M (English)    | `kokoro.csv`            | 28    | `kokoro:all`           |
| Sherpa-ONNX VCTK        | `sherpa_vctk.csv`       | 109   | `all`, ranges (`0-99`) |
| Sherpa-ONNX LibriTTS-R  | `sherpa_libritts_r.csv` | 904   | `all`, ranges (`0-99`) |
| Sherpa-ONNX MeloTTS-en  | `sherpa_melo_en.csv`    | 1     | `all`                  |

Voice string format in the ini is always `<engine>:<name_or_id>`. Sherpa
entries have a nested `<engine>:<model>:<id>` form.

See `dialogue_reader.ini` comments for the exact shorthand rules.
