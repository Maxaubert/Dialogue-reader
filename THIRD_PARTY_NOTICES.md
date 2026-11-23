# Third-party notices

DialogReader includes the third-party code, data and files listed here. This file ships next to `DialogReader.exe`
in the installer (M5, 2026-11-23).

- **Compiled into DialogReader.exe**: the Kokoro code from Sonara, misaki-rs and its data, Slint, Skia, the Geist
  fonts, and every Rust crate in the table below.
- **Next to DialogReader.exe**: ONNX Runtime, the Visual C++ runtime, and the Kokoro model and voices.

The crate table is made by `installer/notices.py` from `Cargo.lock`: every crate linked into the exe (no build or
dev dependencies, no proc-macros), its licence, and the copyright lines of the crate's own licence files at that
version. A crate whose licence files have no copyright line is shown with its Cargo.toml authors.

## Compiled into DialogReader.exe

### Kokoro code from Sonara (MIT)

The Kokoro model, voice, phoneme and text modules in `crates/tts/src/kokoro` are copied from Sonara
(https://github.com/Maxaubert/Sonara, commit `8fd2af726869e1dfe49814ea21eb4aa7bd335c99`).

```text
MIT License

Copyright (c) 2026 Nima Hakimi

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### misaki-rs (MIT)

The English phoneme code: misaki-rs 0.6.0 by Michele Yin (https://github.com/MicheleYin/misaki-rs), as vendored in
Sonara (`crates/misaki`, same commit), a Rust port of misaki.

```text
MIT License

Copyright (c) 2026 Michele Yin

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### misaki lexicons and tagger weights (Apache-2.0)

- English pronunciation lexicons (`us_gold`, `us_silver`, `us_heteronyms`): from misaki by hexgrad
  (https://github.com/hexgrad/misaki), Apache-2.0. Some entries were made with espeak-ng (GPL-3.0) by misaki-rs;
  no espeak-ng code or data is in DialogReader. Their provenance is checked before any paid release.
- Part-of-speech tagger weights: NLTK's `averaged_perceptron_tagger` model, as packaged by postagger.rs
  (https://github.com/shubham0204/postagger.rs, Apache-2.0); the tagger design is from textblob-aptagger by Matthew
  Honnibal (MIT).
- The Kokoro-82M phoneme vocabulary (`vocab` table of its `config.json`, hexgrad, Apache-2.0).

Source of these lines: Sonara's `packaging/notices/models-and-data.md` at the same commit. The Apache-2.0 text is
at the end of this file.

### Slint (UI toolkit)

Slint 1.18.1 by SixtyFPS GmbH (https://slint.dev): the crates `slint`, `i-slint-*` and `vtable` parts listed below.
Slint is offered under GPL-3.0-only OR the Slint Royalty-free Desktop, Mobile, and Web Applications License 2.0 OR
the Slint Software License 3.0. **Which of these DialogReader uses is not decided yet** (docs/app-plan.md, M5 open
questions). The royalty-free licence asks for an attribution: the `AboutSlint` widget in an About screen, or the
"Made with Slint" badge on the public download page. The licence texts are in each Slint crate's `LICENSES` folder.

### Skia (2D graphics)

Skia by Google (https://skia.org), compiled into `skia-bindings` (rust-skia, MIT, listed below): BSD-3-Clause,
Copyright (c) 2011 Google Inc. The BSD-3-Clause text is at the end of this file. Skia's own bundled libraries
(for example FreeType, HarfBuzz, libpng, zlib, expat, ICU) as built by rust-skia 0.153.3 are not listed one by one
yet (docs/app-plan.md, M5 open questions).

### Geist and Geist Mono fonts

The UI fonts in `crates/app/ui/fonts`: Copyright 2026 The Geist Project Authors, SIL Open Font License 1.1 (text at
the end of this file).

### Rust crates

<!-- crates:begin (installer/notices.py writes this part) -->
| Crate | Licence | Copyright |
|---|---|---|
| accesskit 0.24.1 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: The AccessKit contributors |
| accesskit_consumer 0.38.0 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: The AccessKit contributors |
| accesskit_windows 0.34.0 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: The AccessKit contributors |
| accesskit_winit 0.33.2 | Apache-2.0 | no copyright line in its licence files; authors: The AccessKit contributors |
| adler2 2.0.1 | 0BSD OR MIT OR Apache-2.0 | Copyright (C) Jonas Schievink <jonasschievink@gmail.com> |
| aho-corasick 1.1.5 | Unlicense OR MIT | Copyright (c) 2015 Andrew Gallant |
| allocator-api2 0.2.21 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Zakarum |
| annotate-snippets 0.12.18 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| anstyle 1.0.14 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| arboard 3.7.0 | MIT OR Apache-2.0 | Copyright (c) 2022 The Arboard contributors |
| arrayref 0.3.9 | BSD-2-Clause | Copyright (c) 2015 David Roundy <roundyd@physics.oregonstate.edu> |
| arrayvec 0.7.8 | MIT OR Apache-2.0 | Copyright (c) Ulrik Sverdrup "bluss" 2015-2023 |
| base64 0.23.1 | MIT OR Apache-2.0 | Copyright (c) 2025 Alice Maz, Marshall Pierce |
| bit-set 0.10.0 | Apache-2.0 OR MIT | Copyright (c) 2026 The Rust Project Developers |
| bit-vec 0.9.1 | Apache-2.0 OR MIT | Copyright (c) 2023 The Rust Project Developers |
| bitflags 2.13.2 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| block-buffer 0.10.4 | MIT OR Apache-2.0 | Copyright (c) 2018-2019 The RustCrypto Project Developers |
| by_address 1.2.1 | MIT OR Apache-2.0 | Copyright (c) 2017 Matt Brubeck |
| bytemuck 1.25.2 | Zlib OR Apache-2.0 OR MIT | Copyright (c) 2019 Daniel "Lokathor" Gee. |
| byteorder-lite 0.1.0 | Unlicense OR MIT | Copyright (c) 2015 Andrew Gallant |
| cfg-if 1.0.5 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| chrono 0.4.45 | MIT OR Apache-2.0 | Copyright (c) 2014, Kang Seonghoon. |
| clipboard-win 5.4.3 | BSL-1.0 | no copyright line in its licence files; authors: Douman |
| clru 0.6.3 | MIT | Copyright (c) 2020 Élie ROUDNINSKI (marmeladema) <xademax@gmail.com> |
| codespan-reporting 0.13.1 | Apache-2.0 | no copyright line in its licence files; authors: Brendan Zabarauskas |
| color_quant 1.1.0 | MIT | Copyright (c) 2016 PistonDevelopers |
| const-field-offset 0.2.1 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Slint Developers |
| convert_case 0.10.0 | MIT | Copyright (c) 2025 rutrum |
| countme 3.0.1 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Aleksey Kladov |
| cpal 0.16.0 | Apache-2.0 | no copyright line in its licence files |
| cpufeatures 0.2.17 | MIT OR Apache-2.0 | Copyright (c) 2020-2025 The RustCrypto Project Developers |
| crc32fast 1.5.5 | MIT OR Apache-2.0 | Copyright (c) 2018 Sam Rijs, Alex Crichton and contributors |
| critical-section 1.2.0 | MIT OR Apache-2.0 | Copyright (c) 2022 The critical-section authors |
| crossbeam-channel 0.5.17 | MIT OR Apache-2.0 | Copyright (c) 2019 The Crossbeam Project Developers; Copyright (c) 2009 The Go Authors. All rights reserved. |
| crossbeam-deque 0.8.8 | MIT OR Apache-2.0 | Copyright (c) 2019 The Crossbeam Project Developers |
| crossbeam-epoch 0.9.21 | MIT OR Apache-2.0 | Copyright (c) 2019 The Crossbeam Project Developers |
| crossbeam-utils 0.8.23 | MIT OR Apache-2.0 | Copyright (c) 2019 The Crossbeam Project Developers |
| crypto-common 0.1.7 | MIT OR Apache-2.0 | Copyright (c) 2021 RustCrypto Developers |
| cursor-icon 1.2.0 | MIT OR Apache-2.0 OR Zlib | Copyright 2023 Kirill Chibisov; Copyright (c) 2023 Kirill Chibisov |
| dasp_sample 0.11.0 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: mitchmindtree |
| data-url 0.3.2 | MIT OR Apache-2.0 | Copyright (c) 2013-2025 The rust-url developers |
| derive_more 2.1.1 | MIT | Copyright (c) 2016 Jelte Fennema |
| digest 0.10.7 | MIT OR Apache-2.0 | Copyright (c) 2017 Artyom Pavlov |
| dpi 0.1.2 | Apache-2.0 AND MIT | Copyright (c) 2018 Jorge Aparicio; Copyright © 2005-2020 Rich Felker, et al.; Copyright © 1993,2004 Sun Microsystems or; Copyright © 2003-2011 David Schultz or; Copyright © 2003-2009 Steven G. Kargl or; Copyright © 2003-2009 Bruce D. Evans or; Copyright © 2008 Stephen L. Moshier or; Copyright © 2017-2018 Arm Limited |
| either 1.19.0 | MIT OR Apache-2.0 | Copyright (c) 2015 |
| equivalent 1.0.3 | Apache-2.0 OR MIT | Copyright (c) 2016--2023 |
| error-code 3.4.0 | BSL-1.0 | no copyright line in its licence files; authors: Douman |
| euclid 0.22.14 | MIT OR Apache-2.0 | Copyright (c) 2012-2013 Mozilla Foundation |
| fdeflate 0.3.7 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: The image-rs Developers |
| field-offset 0.3.6 | MIT OR Apache-2.0 | Copyright (c) 2016-2021 Diggory Blake, and other contributors. |
| fixed_decimal 0.7.3 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| flate2 1.1.11 | MIT OR Apache-2.0 | Copyright (c) 2014-2026 Alex Crichton |
| float-cmp 0.9.0 | MIT | Copyright (c) 2014-2020 Optimal Computing (NZ) Ltd |
| foldhash 0.2.0 | Zlib | Copyright (c) 2024 Orson Peters |
| font-types 0.12.7 | MIT OR Apache-2.0 | Copyright 2019 Fontations Developers; Copyright (c) 2019 Fontations Developers |
| fontdb 0.24.0 | MIT | Copyright (c) 2020 Yevhenii Reizner |
| fontique 0.11.1 | Apache-2.0 OR MIT | Copyright 2024 the Parley Authors |
| form_urlencoded 1.2.3 | MIT OR Apache-2.0 | Copyright (c) 2013-2016 The rust-url developers |
| generic-array 0.14.7 | MIT | Copyright (c) 2015 Bartłomiej Kamiński |
| gif 0.14.2 | MIT OR Apache-2.0 | Copyright (c) 2015 nwin |
| glow 0.18.0 | MIT OR Apache-2.0 OR Zlib | no copyright line in its licence files; authors: Joshua Groves, Dzmitry Malyshau |
| glutin 0.32.3 | Apache-2.0 | Copyright 2022 Kirill Chibisov |
| glutin_egl_sys 0.7.1 | Apache-2.0 | Copyright 2022 Kirill Chibisov |
| glutin_wgl_sys 0.6.1 | Apache-2.0 | Copyright 2022 Kirill Chibisov |
| gpu-allocator 0.28.0 | MIT OR Apache-2.0 | Copyright 2021 Traverse Research B.V.; Copyright (c) 2021 Traverse Research B.V. |
| half 2.7.1 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Kathryn Long |
| harfrust 0.12.0 | MIT | Copyright (c) HarfBuzz developers; Copyright (c) 2020 Yevhenii Reizner |
| hashbrown 0.14.5 | MIT OR Apache-2.0 | Copyright (c) 2016 Amanieu d'Antras |
| hashbrown 0.16.1 | MIT OR Apache-2.0 | Copyright (c) 2016 Amanieu d'Antras |
| hashbrown 0.17.2 | MIT OR Apache-2.0 | Copyright (c) 2016 Amanieu d'Antras |
| heck 0.5.0 | MIT OR Apache-2.0 | Copyright (c) 2015 The Rust Project Developers |
| htmlparser 0.2.1 | MIT OR Apache-2.0 | Copyright (c) 2018 Reizner Evgeniy |
| i-slint-backend-selector 1.18.1 | GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 | no copyright line in its licence files; authors: Slint Developers |
| i-slint-backend-winit 1.18.1 | GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 | no copyright line in its licence files; authors: Slint Developers |
| i-slint-common 1.18.1 | GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 | no copyright line in its licence files; authors: Slint Developers |
| i-slint-compiler 1.18.1 | GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 | no copyright line in its licence files; authors: Slint Developers |
| i-slint-core 1.18.1 | GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 | no copyright line in its licence files; authors: Slint Developers |
| i-slint-renderer-skia 1.18.1 | GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 | no copyright line in its licence files; authors: Slint Developers |
| i-slint-renderer-software 1.18.1 | GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 | no copyright line in its licence files; authors: Slint Developers |
| icu_collections 2.4.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| icu_decimal 2.4.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| icu_decimal_data 2.4.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| icu_locale_core 2.4.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| icu_locale_fallback 2.4.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| icu_locale_fallback_data 2.4.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| icu_normalizer 2.4.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| icu_normalizer_data 2.4.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| icu_properties 2.4.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| icu_properties_data 2.4.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| icu_provider 2.4.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| icu_segmenter 2.4.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| icu_segmenter_data 2.4.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| idna 1.1.0 | MIT OR Apache-2.0 | Copyright (c) 2013-2025 The rust-url developers |
| idna_adapter 1.2.2 | Apache-2.0 OR MIT | Copyright (c) The rust-url developers |
| image 0.25.10 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: The image-rs Developers |
| image-webp 0.2.4 | MIT OR Apache-2.0 | no copyright line in its licence files |
| imagesize 0.15.0 | MIT | Copyright (c) 2017 Maiddog |
| imgref 1.12.3 | CC0-1.0 OR Apache-2.0 | no copyright line in its licence files; authors: Kornel Lesiński |
| indexmap 2.14.2 | Apache-2.0 OR MIT | Copyright (c) 2016--2017 |
| itertools 0.15.0 | MIT OR Apache-2.0 | Copyright (c) 2015 |
| itoa 1.0.19 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: David Tolnay |
| keyboard-types 0.7.0 | MIT OR Apache-2.0 | Copyright (c) 2017 Pyfisch |
| kurbo 0.13.1 | Apache-2.0 OR MIT | Copyright (c) 2018 Raph Levien |
| lazy_static 1.5.1 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Marvin Löbel |
| libloading 0.8.9 | ISC | Copyright © 2015, Simonas Kazlauskas |
| libloading 0.9.1 | ISC | Copyright © 2015, Simonas Kazlauskas |
| libm 0.2.16 | MIT | Copyright (c) 2018 Jorge Aparicio; Copyright © 2005-2020 Rich Felker, et al.; Copyright © 1993,2004 Sun Microsystems or; Copyright © 2003-2011 David Schultz or; Copyright © 2003-2009 Steven G. Kargl or; Copyright © 2003-2009 Bruce D. Evans or; Copyright © 2008 Stephen L. Moshier or; Copyright © 2017-2018 Arm Limited |
| linebender_resource_handle 0.1.1 | Apache-2.0 OR MIT | no copyright line in its licence files |
| litemap 0.8.3 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| litrs 1.0.1 | MIT OR Apache-2.0 | Copyright (c) 2020 Project Developers |
| lock_api 0.4.14 | MIT OR Apache-2.0 | Copyright (c) 2016 The Rust Project Developers |
| log 0.4.35 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| lyon_algorithms 1.0.22 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Nicolas Silva |
| lyon_extra 1.1.0 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Nicolas Silva |
| lyon_geom 1.0.20 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Nicolas Silva |
| lyon_path 1.0.19 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Nicolas Silva |
| lzma-rust2 0.15.8 | Apache-2.0 | no copyright line in its licence files |
| memchr 2.8.3 | Unlicense OR MIT | Copyright (c) 2015 Andrew Gallant |
| memmap2 0.9.12 | MIT OR Apache-2.0 | Copyright (c) 2020 Yevhenii Reizner; Copyright (c) 2015 Dan Burkert |
| memoffset 0.9.1 | MIT | Copyright (c) 2017 Gilad Naaman |
| miniz_oxide 0.8.9 | MIT OR Zlib OR Apache-2.0 | Copyright 2013-2014 RAD Game Tools and Valve Software; Copyright 2010-2014 Rich Geldreich and Tenacious Software LLC; Copyright (c) 2017 Frommi; Copyright (c) 2017-2024 oyvindln; Copyright (c) 2020 Frommi |
| miniz_oxide 0.9.1 | MIT OR Zlib OR Apache-2.0 | Copyright 2013-2014 RAD Game Tools and Valve Software; Copyright 2010-2014 Rich Geldreich and Tenacious Software LLC; Copyright (c) 2017 Frommi; Copyright (c) 2017-2024 oyvindln; Copyright (c) 2020 Frommi |
| misaki 0.6.0-sonara.1 | MIT | Copyright (c) 2026 Michele Yin |
| moxcms 0.8.1 | BSD-3-Clause OR Apache-2.0 | Copyright 2024 Radzivon Bartoshyk; Copyright (c) Radzivon Bartoshyk. All rights reserved. |
| muda 0.19.3 | Apache-2.0 OR MIT | Copyright (c) 2022-2022 Tauri Programme within The Commons Conservancy |
| naga 30.0.2 | MIT OR Apache-2.0 | Copyright (c) 2025 The gfx-rs developers |
| naga-types 30.0.2 | MIT OR Apache-2.0 | Copyright (c) 2025 The gfx-rs developers |
| natord 1.0.9 | MIT | Copyright (c) 2014, Kang Seonghoon. |
| num-bigint 0.4.8 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| num-integer 0.1.47 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| num-rational 0.4.2 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| num-traits 0.2.19 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| num2words 0.4.1 | MIT OR Apache-2.0 | Copyright (c) 2022 Asperatus |
| num_enum 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | Copyright (c) 2018, Daniel Wagner-Hall |
| once_cell 1.21.4 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Aleksey Kladov |
| ordered-float 5.5.1 | MIT | Copyright (c) 2015 Jonathan Reem |
| ort 2.0.0-rc.15 | MIT OR Apache-2.0 | Copyright (c) 2023-2026 pyke.io; Copyright (c) 2020 Nicolas Bigaouette |
| ort-sys 2.0.0-rc.15 | MIT OR Apache-2.0 | Copyright (c) 2023-2026 pyke.io; Copyright (c) 2020 Nicolas Bigaouette |
| parking_lot 0.12.5 | MIT OR Apache-2.0 | Copyright (c) 2016 The Rust Project Developers |
| parking_lot_core 0.9.12 | MIT OR Apache-2.0 | Copyright (c) 2016 The Rust Project Developers |
| parlance 0.1.1 | Apache-2.0 OR MIT | Copyright 2020 the Parley Authors |
| parley 0.11.1 | Apache-2.0 OR MIT | Copyright 2020 the Parley Authors |
| parley_data 0.11.1 | Apache-2.0 OR MIT | Copyright 2020 the Parley Authors |
| percent-encoding 2.3.2 | MIT OR Apache-2.0 | Copyright (c) 2013-2025 The rust-url developers |
| pico-args 0.5.0 | MIT | Copyright (c) 2019 Yevhenii Reizner |
| pin-project 1.2.0 | Apache-2.0 OR MIT | no copyright line in its licence files |
| pin-project-lite 0.2.18 | Apache-2.0 OR MIT | no copyright line in its licence files |
| pin-utils 0.1.1 | MIT OR Apache-2.0 | Copyright 2018 The pin-utils authors; Copyright (c) 2018 The pin-utils authors |
| pin-weak 1.1.0 | MIT | Copyright (c) 2020 Olivier Goffart <ogoffart@sixtyfps.io> |
| png 0.18.1 | MIT OR Apache-2.0 | Copyright (c) 2015 nwin |
| polycool 0.4.0 | MIT OR Apache-2.0 | Copyright (c) 2018 Raph Levien |
| portable-atomic 1.15.0 | Apache-2.0 OR MIT | no copyright line in its licence files |
| potential_utf 0.1.6 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| presser 0.3.1 | MIT OR Apache-2.0 | Copyright (c) 2019 Embark Studios |
| proc-macro-crate 3.5.0 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Bastian Köcher |
| proc-macro2 1.0.107 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: David Tolnay, Alex Crichton |
| profiling 1.0.18 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Philip Degarmo |
| pulldown-cmark 0.13.6 | MIT | Copyright 2015 Google Inc. All rights reserved. |
| pxfm 0.1.30 | BSD-3-Clause OR Apache-2.0 | Copyright 2024 Radzivon Bartoshyk; Copyright (c) Radzivon Bartoshyk. All rights reserved. |
| quick-error 2.0.1 | MIT/Apache-2.0 | Copyright (c) 2015 The quick-error Developers |
| quote 1.0.47 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: David Tolnay |
| range-alloc 0.1.5 | MIT OR Apache-2.0 | Copyright (c) 2023 The gfx-rs developers |
| raw-window-handle 0.6.2 | MIT OR Apache-2.0 OR Zlib | Copyright (c) 2019 Osspial; Copyright (c) 2020 Osspial |
| rayon 1.12.0 | MIT OR Apache-2.0 | Copyright (c) 2010 The Rust Project Developers |
| rayon-core 1.13.0 | MIT OR Apache-2.0 | Copyright (c) 2010 The Rust Project Developers |
| read-fonts 0.41.0 | MIT OR Apache-2.0 | Copyright 2019 Fontations Developers; Copyright (c) 2019 Fontations Developers |
| regex 1.13.2 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| regex-automata 0.4.19 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| regex-syntax 0.8.12 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| renderdoc-sys 1.1.0 | MIT OR Apache-2.0 | Copyright (c) 2022 Eyal Kalderon |
| resvg 0.48.1 | Apache-2.0 OR MIT | Copyright 2017 the Resvg Authors |
| rgb 0.8.53 | MIT | Copyright (c) 2019 Kornel |
| rodio 0.21.1 | MIT OR Apache-2.0 | Copyright 2015 Pierre Krieger |
| rowan 0.17.0 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Aleksey Kladov |
| roxmltree 0.21.2 | MIT OR Apache-2.0 | Copyright (c) 2018 Yevhenii Reizner |
| rspolib 0.1.2 | MIT | no copyright line in its licence files |
| rustc-hash 1.1.0 | Apache-2.0/MIT | no copyright line in its licence files; authors: The Rust Project Developers |
| scoped-tls-hkt 0.1.5 | MIT/Apache-2.0 | Copyright (c) 2020 Diggory Blake |
| scopeguard 1.2.0 | MIT OR Apache-2.0 | Copyright (c) 2016-2019 Ulrik Sverdrup "bluss" and scopeguard developers |
| serde 1.0.229 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Erick Tryzelaar, David Tolnay |
| serde_core 1.0.229 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Erick Tryzelaar, David Tolnay |
| serde_json 1.0.154 | MIT OR Apache-2.0 | no copyright line in its licence files |
| sha2 0.10.9 | MIT OR Apache-2.0 | Copyright (c) 2006-2009 Graydon Hoare; Copyright (c) 2009-2013 Mozilla Foundation; Copyright (c) 2016 Artyom Pavlov |
| simd-adler32 0.3.10 | MIT | Copyright (c) [2021] [Marvin Countryman] |
| simplecss 0.2.2 | Apache-2.0 OR MIT | Copyright (c) 2018 Reizner Evgeniy |
| siphasher 1.0.4 | MIT OR Apache-2.0 | Copyright 2012-2016 The Rust Project Developers.; Copyright 2016-2026 Frank Denis. |
| skia-bindings 0.153.3 | MIT | no copyright line in its licence files; authors: LongYinan, Armin Sander |
| skia-safe 0.153.3 | MIT | no copyright line in its licence files; authors: Armin Sander |
| skrifa 0.44.0 | MIT OR Apache-2.0 | Copyright 2019 Fontations Developers; Copyright (c) 2019 Fontations Developers |
| slab 0.4.12 | MIT | Copyright (c) 2019 Carl Lerche |
| slint 1.18.1 | GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 | no copyright line in its licence files; authors: Slint Developers |
| slotmap 1.1.1 | Zlib | Copyright (c) 2021 Orson Peters <orsonpeters@gmail.com> |
| smallvec 1.16.6 | MIT OR Apache-2.0 | Copyright (c) 2018 The Servo Project Developers |
| smol_str 0.2.2 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Aleksey Kladov |
| smol_str 0.3.7 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Aleksey Kladov, Lukas Wirth |
| snafu 0.8.9 | MIT OR Apache-2.0 | Copyright 2019- Jake Goulding; Copyright (c) 2019- Jake Goulding |
| softbuffer 0.4.8 | MIT OR Apache-2.0 | Copyright 2022 Kirill Chibisov |
| spin_on 0.1.1 | Apache-2.0 OR MIT | no copyright line in its licence files; authors: paulkernfeld |
| stable_deref_trait 1.2.1 | MIT OR Apache-2.0 | Copyright (c) 2017 Robert Grosse |
| static_assertions 1.1.0 | MIT OR Apache-2.0 | Copyright (c) 2017 Nikolai Vazquez |
| strict-num 0.1.1 | MIT | Copyright (c) 2022 Yevhenii Reizner |
| strum 0.28.0 | MIT | Copyright (c) 2019 Peter Glotfelty |
| svgtypes 0.16.2 | Apache-2.0 OR MIT | Copyright (c) 2018 Yevhenii Reizner |
| swash 0.2.10 | Apache-2.0 OR MIT | Copyright (c) 2020 Chad Brokaw |
| syn 2.0.119 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: David Tolnay |
| syn 3.0.10 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: David Tolnay |
| synstructure 0.14.0 | MIT | Copyright 2016 Nika Layzell |
| sys-locale 0.3.2 | MIT OR Apache-2.0 | Copyright (c) 2021 1Password |
| taffy 0.10.1 | MIT | no copyright line in its licence files; authors: Alice Cecile, Johnathan Kelley, Nico Burns |
| text-size 1.1.1 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Aleksey Kladov, Christopher Durham (CAD97) |
| thiserror 2.0.21 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: David Tolnay |
| tiny-skia 0.12.0 | BSD-3-Clause | Copyright (c) 2011 Google Inc. All rights reserved.; Copyright (c) 2020 Yevhenii Reizner All rights reserved. |
| tiny-skia-path 0.12.0 | BSD-3-Clause | Copyright (c) 2011 Google Inc. All rights reserved.; Copyright (c) 2020 Yevhenii Reizner All rights reserved. |
| tinystr 0.8.5 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| tinyvec 1.13.3 | Zlib OR Apache-2.0 OR MIT | Copyright (c) 2019 Daniel "Lokathor" Gee. |
| toml_datetime 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| toml_edit 0.25.20+spec-1.1.0 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| toml_parser 1.1.6+spec-1.1.0 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| toml_writer 1.2.0+spec-1.1.0 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| tracing 0.1.44 | MIT | Copyright (c) 2019 Tokio Contributors |
| tracing-core 0.1.36 | MIT | Copyright (c) 2019 Tokio Contributors |
| typed-index-collections 3.5.0 | MIT OR Apache-2.0 | Copyright (c) 2020 Andrey Zheleznov <zheland.net@gmail.com> |
| typenum 1.20.1 | MIT OR Apache-2.0 | Copyright 2014 Paho Lurie-Gregg; Copyright (c) 2014 Paho Lurie-Gregg |
| unicase 2.10.0 | MIT OR Apache-2.0 | Copyright (c) 2014-2026 Sean McArthur |
| unicode-bidi 0.3.18 | MIT OR Apache-2.0 | Copyright (c) 2015 The Rust Project Developers |
| unicode-ident 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | Copyright © 1991-2023 Unicode, Inc. |
| unicode-linebreak 0.1.5 | Apache-2.0 | no copyright line in its licence files; authors: Axel Forsman |
| unicode-script 0.5.9 | MIT OR Apache-2.0 | Copyright 2021 The Unicode-rs Developers; Copyright (c) 2019 Manish Goregaokar |
| unicode-segmentation 1.13.3 | MIT OR Apache-2.0 | Copyright (c) 2015 The Rust Project Developers |
| unicode-vo 0.1.0 | MIT/Apache-2.0 | Copyright (c) 2018 Reizner Evgeniy |
| unicode-width 0.2.3 | MIT OR Apache-2.0 | Copyright (c) 2015 The Rust Project Developers |
| unicode-xid 0.2.6 | MIT OR Apache-2.0 | Copyright (c) 2015 The Rust Project Developers |
| url 2.5.9 | MIT OR Apache-2.0 | Copyright (c) 2013-2025 The rust-url developers |
| usvg 0.48.1 | Apache-2.0 OR MIT | Copyright 2017 the Resvg Authors |
| utf8_iter 1.1.0 | Apache-2.0 OR MIT | no copyright line in its licence files; authors: Henri Sivonen |
| utf_types 1.0.0 | Apache-2.0 OR MIT | no copyright line in its licence files; authors: Henri Sivonen |
| uuid 1.29.3 | Apache-2.0 OR MIT | Copyright (c) 2014 The Rust Project Developers; Copyright (c) 2018 Ashley Mannix, Christopher Armstrong, Dylan DPC, Hunar Roop Kahlon |
| vtable 0.5.1 | MIT OR Apache-2.0 | no copyright line in its licence files; authors: Slint Developers |
| webbrowser 1.2.7 | MIT OR Apache-2.0 | Copyright (c) 2015-2022 Amod Malviya |
| weezl 0.1.12 | MIT OR Apache-2.0 | Copyright (c) HeroicKatora 2020 |
| wgpu 30.0.2 | MIT OR Apache-2.0 | Copyright (c) 2025 The gfx-rs developers |
| wgpu-core 30.0.2 | MIT OR Apache-2.0 | Copyright (c) 2025 The gfx-rs developers |
| wgpu-core-deps-windows-linux-android 30.0.2 | MIT OR Apache-2.0 | Copyright (c) 2025 The gfx-rs developers |
| wgpu-hal 30.0.2 | MIT OR Apache-2.0 | Copyright (c) 2025 The gfx-rs developers |
| wgpu-naga-bridge 30.0.2 | MIT OR Apache-2.0 | Copyright (c) 2025 The gfx-rs developers |
| wgpu-types 30.0.2 | MIT OR Apache-2.0 | Copyright (c) 2025 The gfx-rs developers |
| windows 0.54.0 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| windows 0.62.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| windows-capture 2.0.1 | MIT | Copyright (c) 2023 NiiightmareXD |
| windows-collections 0.3.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| windows-core 0.54.0 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| windows-core 0.62.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| windows-future 0.3.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| windows-link 0.2.1 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| windows-numerics 0.3.1 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| windows-result 0.1.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| windows-result 0.4.1 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| windows-strings 0.5.1 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| windows-sys 0.52.0 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| windows-sys 0.61.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| windows-targets 0.52.6 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| windows-threading 0.2.1 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| windows_x86_64_msvc 0.52.6 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| winit 0.30.13 | Apache-2.0 | no copyright line in its licence files; authors: The winit contributors, Pierre Krieger |
| winnow 1.0.5 | MIT | no copyright line in its licence files |
| writeable 0.6.4 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| xmlwriter 0.1.0 | MIT | Copyright (c) 2019 Reizner Evgeniy |
| yazi 0.2.2 | Apache-2.0 OR MIT | Copyright (c) 2020 Chad Brokaw |
| yoke 0.8.4 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| zeno 0.3.3 | Apache-2.0 OR MIT | Copyright (c) 2020 Chad Brokaw |
| zerocopy 0.8.64 | BSD-2-Clause OR Apache-2.0 OR MIT | Copyright 2023 The Fuchsia Authors; Copyright 2019 The Fuchsia Authors. |
| zerofrom 0.1.8 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| zerotrie 0.2.5 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| zerovec 0.11.9 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| zmij 1.0.23 | MIT | no copyright line in its licence files; authors: David Tolnay |
| zune-core 0.5.3 | MIT OR Apache-2.0 OR Zlib | Copyright (c) zune-image developers |
| zune-jpeg 0.5.15 | MIT OR Apache-2.0 OR Zlib | Copyright (c) zune-image developers |
<!-- crates:end -->

## Next to DialogReader.exe

The installer puts these files in the app folder (Kokoro is bundled, Max, 2026-11-23). Texts in Sonara's
`packaging/notices/models-and-data.md` and the files named here.

- `kokoro\kokoro-v1.0.onnx` and `kokoro\voices-v1.0.bin`: Kokoro-82M v1.0 weights by hexgrad, Apache-2.0
  (https://huggingface.co/hexgrad/Kokoro-82M); the ONNX export and the voices file from kokoro-onnx by thewh1teagle
  (https://github.com/thewh1teagle/kokoro-onnx, release `model-files-v1.0`), MIT.
- `onnxruntime.dll` 1.28.2: Microsoft ONNX Runtime, MIT, Copyright (c) Microsoft Corporation. Its licence and its
  third-party notices ship next to it as `onnxruntime-LICENSE.txt` and `onnxruntime-ThirdPartyNotices.txt`.
- `msvcp140.dll`, `msvcp140_1.dll`, `vcruntime140.dll`, `vcruntime140_1.dll`: Microsoft Visual C++ runtime,
  Distributable Code under the Microsoft Visual Studio licence terms (app-local deployment).

## Licence texts

### MIT

The MIT texts above (Sonara, misaki-rs) are the same licence; each MIT crate in the table is under it with its own
copyright line. Crates offered under several licences (for example "MIT OR Apache-2.0") are used under the MIT
licence where it is one of the choices, otherwise under the first licence listed whose text is below.

### ISC (libloading)

```text
Copyright © 2015, Simonas Kazlauskas

Permission to use, copy, modify, and/or distribute this software for any purpose with or without
fee is hereby granted, provided that the above copyright notice and this permission notice appear
in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS
SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE
AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT,
NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF
THIS SOFTWARE.
```

### Apache License 2.0

```text
                              Apache License
                        Version 2.0, January 2004
                     http://www.apache.org/licenses/

TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION

1. Definitions.

   "License" shall mean the terms and conditions for use, reproduction,
   and distribution as defined by Sections 1 through 9 of this document.

   "Licensor" shall mean the copyright owner or entity authorized by
   the copyright owner that is granting the License.

   "Legal Entity" shall mean the union of the acting entity and all
   other entities that control, are controlled by, or are under common
   control with that entity. For the purposes of this definition,
   "control" means (i) the power, direct or indirect, to cause the
   direction or management of such entity, whether by contract or
   otherwise, or (ii) ownership of fifty percent (50%) or more of the
   outstanding shares, or (iii) beneficial ownership of such entity.

   "You" (or "Your") shall mean an individual or Legal Entity
   exercising permissions granted by this License.

   "Source" form shall mean the preferred form for making modifications,
   including but not limited to software source code, documentation
   source, and configuration files.

   "Object" form shall mean any form resulting from mechanical
   transformation or translation of a Source form, including but
   not limited to compiled object code, generated documentation,
   and conversions to other media types.

   "Work" shall mean the work of authorship, whether in Source or
   Object form, made available under the License, as indicated by a
   copyright notice that is included in or attached to the work
   (an example is provided in the Appendix below).

   "Derivative Works" shall mean any work, whether in Source or Object
   form, that is based on (or derived from) the Work and for which the
   editorial revisions, annotations, elaborations, or other modifications
   represent, as a whole, an original work of authorship. For the purposes
   of this License, Derivative Works shall not include works that remain
   separable from, or merely link (or bind by name) to the interfaces of,
   the Work and Derivative Works thereof.

   "Contribution" shall mean any work of authorship, including
   the original version of the Work and any modifications or additions
   to that Work or Derivative Works thereof, that is intentionally
   submitted to Licensor for inclusion in the Work by the copyright owner
   or by an individual or Legal Entity authorized to submit on behalf of
   the copyright owner. For the purposes of this definition, "submitted"
   means any form of electronic, verbal, or written communication sent
   to the Licensor or its representatives, including but not limited to
   communication on electronic mailing lists, source code control systems,
   and issue tracking systems that are managed by, or on behalf of, the
   Licensor for the purpose of discussing and improving the Work, but
   excluding communication that is conspicuously marked or otherwise
   designated in writing by the copyright owner as "Not a Contribution."

   "Contributor" shall mean Licensor and any individual or Legal Entity
   on behalf of whom a Contribution has been received by Licensor and
   subsequently incorporated within the Work.

2. Grant of Copyright License. Subject to the terms and conditions of
   this License, each Contributor hereby grants to You a perpetual,
   worldwide, non-exclusive, no-charge, royalty-free, irrevocable
   copyright license to reproduce, prepare Derivative Works of,
   publicly display, publicly perform, sublicense, and distribute the
   Work and such Derivative Works in Source or Object form.

3. Grant of Patent License. Subject to the terms and conditions of
   this License, each Contributor hereby grants to You a perpetual,
   worldwide, non-exclusive, no-charge, royalty-free, irrevocable
   (except as stated in this section) patent license to make, have made,
   use, offer to sell, sell, import, and otherwise transfer the Work,
   where such license applies only to those patent claims licensable
   by such Contributor that are necessarily infringed by their
   Contribution(s) alone or by combination of their Contribution(s)
   with the Work to which such Contribution(s) was submitted. If You
   institute patent litigation against any entity (including a
   cross-claim or counterclaim in a lawsuit) alleging that the Work
   or a Contribution incorporated within the Work constitutes direct
   or contributory patent infringement, then any patent licenses
   granted to You under this License for that Work shall terminate
   as of the date such litigation is filed.

4. Redistribution. You may reproduce and distribute copies of the
   Work or Derivative Works thereof in any medium, with or without
   modifications, and in Source or Object form, provided that You
   meet the following conditions:

   (a) You must give any other recipients of the Work or
       Derivative Works a copy of this License; and

   (b) You must cause any modified files to carry prominent notices
       stating that You changed the files; and

   (c) You must retain, in the Source form of any Derivative Works
       that You distribute, all copyright, patent, trademark, and
       attribution notices from the Source form of the Work,
       excluding those notices that do not pertain to any part of
       the Derivative Works; and

   (d) If the Work includes a "NOTICE" text file as part of its
       distribution, then any Derivative Works that You distribute must
       include a readable copy of the attribution notices contained
       within such NOTICE file, excluding those notices that do not
       pertain to any part of the Derivative Works, in at least one
       of the following places: within a NOTICE text file distributed
       as part of the Derivative Works; within the Source form or
       documentation, if provided along with the Derivative Works; or,
       within a display generated by the Derivative Works, if and
       wherever such third-party notices normally appear. The contents
       of the NOTICE file are for informational purposes only and
       do not modify the License. You may add Your own attribution
       notices within Derivative Works that You distribute, alongside
       or as an addendum to the NOTICE text from the Work, provided
       that such additional attribution notices cannot be construed
       as modifying the License.

   You may add Your own copyright statement to Your modifications and
   may provide additional or different license terms and conditions
   for use, reproduction, or distribution of Your modifications, or
   for any such Derivative Works as a whole, provided Your use,
   reproduction, and distribution of the Work otherwise complies with
   the conditions stated in this License.

5. Submission of Contributions. Unless You explicitly state otherwise,
   any Contribution intentionally submitted for inclusion in the Work
   by You to the Licensor shall be under the terms and conditions of
   this License, without any additional terms or conditions.
   Notwithstanding the above, nothing herein shall supersede or modify
   the terms of any separate license agreement you may have executed
   with Licensor regarding such Contributions.

6. Trademarks. This License does not grant permission to use the trade
   names, trademarks, service marks, or product names of the Licensor,
   except as required for reasonable and customary use in describing the
   origin of the Work and reproducing the content of the NOTICE file.

7. Disclaimer of Warranty. Unless required by applicable law or
   agreed to in writing, Licensor provides the Work (and each
   Contributor provides its Contributions) on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or
   implied, including, without limitation, any warranties or conditions
   of TITLE, NON-INFRINGEMENT, MERCHANTABILITY, or FITNESS FOR A
   PARTICULAR PURPOSE. You are solely responsible for determining the
   appropriateness of using or redistributing the Work and assume any
   risks associated with Your exercise of permissions under this License.

8. Limitation of Liability. In no event and under no legal theory,
   whether in tort (including negligence), contract, or otherwise,
   unless required by applicable law (such as deliberate and grossly
   negligent acts) or agreed to in writing, shall any Contributor be
   liable to You for damages, including any direct, indirect, special,
   incidental, or consequential damages of any character arising as a
   result of this License or out of the use or inability to use the
   Work (including but not limited to damages for loss of goodwill,
   work stoppage, computer failure or malfunction, or any and all
   other commercial damages or losses), even if such Contributor
   has been advised of the possibility of such damages.

9. Accepting Warranty or Additional Liability. While redistributing
   the Work or Derivative Works thereof, You may choose to offer,
   and charge a fee for, acceptance of support, warranty, indemnity,
   or other liability obligations and/or rights consistent with this
   License. However, in accepting such obligations, You may act only
   on Your own behalf and on Your sole responsibility, not on behalf
   of any other Contributor, and only if You agree to indemnify,
   defend, and hold each Contributor harmless for any liability
   incurred by, or claims asserted against, such Contributor by reason
   of your accepting any such warranty or additional liability.

END OF TERMS AND CONDITIONS
```

### BSD-3-Clause (tiny-skia, tiny-skia-path; same terms for Skia)

The text as tiny-skia ships it. Each BSD-3-Clause crate is under these terms with its own copyright line.

```text
Copyright (c) 2011 Google Inc. All rights reserved.
Copyright (c) 2020 Yevhenii Reizner All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

  * Redistributions of source code must retain the above copyright
    notice, this list of conditions and the following disclaimer.

  * Redistributions in binary form must reproduce the above copyright
    notice, this list of conditions and the following disclaimer in
    the documentation and/or other materials provided with the
    distribution.

  * Neither the name of the copyright holder nor the names of its
    contributors may be used to endorse or promote products derived
    from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### BSD-2-Clause (arrayref)

```text
Copyright (c) 2015 David Roundy <roundyd@physics.oregonstate.edu>
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

1. Redistributions of source code must retain the above copyright
   notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright
   notice, this list of conditions and the following disclaimer in the
   documentation and/or other materials provided with the
   distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### Zlib (foldhash, slotmap)

The text as foldhash ships it; slotmap is under the same terms with its own copyright line.

```text
Copyright (c) 2024 Orson Peters

This software is provided 'as-is', without any express or implied warranty. In
no event will the authors be held liable for any damages arising from the use of
this software.

Permission is granted to anyone to use this software for any purpose, including
commercial applications, and to alter it and redistribute it freely, subject to
the following restrictions:

1. The origin of this software must not be misrepresented; you must not claim
    that you wrote the original software. If you use this software in a product,
    an acknowledgment in the product documentation would be appreciated but is
    not required.

2. Altered source versions must be plainly marked as such, and must not be
    misrepresented as being the original software.

3. This notice may not be removed or altered from any source distribution.
```

### Boost Software License 1.0 (clipboard-win, error-code)

```text
Boost Software License - Version 1.0 - August 17th, 2003

Permission is hereby granted, free of charge, to any person or organization
obtaining a copy of the software and accompanying documentation covered by
this license (the "Software") to use, reproduce, display, distribute,
execute, and transmit the Software, and to prepare derivative works of the
Software, and to permit third-parties to whom the Software is furnished to
do so, all subject to the following:

The copyright notices in the Software and this entire statement, including
the above license grant, this restriction and the following disclaimer,
must be included in all copies of the Software, in whole or in part, and
all derivative works of the Software, unless such copies or derivative
works are solely in the form of machine-executable object code generated by
a source language processor.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE, TITLE AND NON-INFRINGEMENT. IN NO EVENT
SHALL THE COPYRIGHT HOLDERS OR ANYONE DISTRIBUTING THE SOFTWARE BE LIABLE
FOR ANY DAMAGES OR OTHER LIABILITY, WHETHER IN CONTRACT, TORT OR OTHERWISE,
ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
```

### Unicode License v3 (the icu and related crates)

```text
UNICODE LICENSE V3

COPYRIGHT AND PERMISSION NOTICE

Copyright © 2020-2024 Unicode, Inc.

NOTICE TO USER: Carefully read the following legal agreement. BY
DOWNLOADING, INSTALLING, COPYING OR OTHERWISE USING DATA FILES, AND/OR
SOFTWARE, YOU UNEQUIVOCALLY ACCEPT, AND AGREE TO BE BOUND BY, ALL OF THE
TERMS AND CONDITIONS OF THIS AGREEMENT. IF YOU DO NOT AGREE, DO NOT
DOWNLOAD, INSTALL, COPY, DISTRIBUTE OR USE THE DATA FILES OR SOFTWARE.

Permission is hereby granted, free of charge, to any person obtaining a
copy of data files and any associated documentation (the "Data Files") or
software and any associated documentation (the "Software") to deal in the
Data Files or Software without restriction, including without limitation
the rights to use, copy, modify, merge, publish, distribute, and/or sell
copies of the Data Files or Software, and to permit persons to whom the
Data Files or Software are furnished to do so, provided that either (a)
this copyright and permission notice appear with all copies of the Data
Files or Software, or (b) this copyright and permission notice appear in
associated Documentation.

THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF ANY
KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF
THIRD PARTY RIGHTS.

IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN THIS NOTICE
BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR CONSEQUENTIAL DAMAGES,
OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS,
WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION,
ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THE DATA
FILES OR SOFTWARE.

Except as contained in this notice, the name of a copyright holder shall
not be used in advertising or otherwise to promote the sale, use or other
dealings in these Data Files or Software without prior written
authorization of the copyright holder.

SPDX-License-Identifier: Unicode-3.0

---

Portions of ICU4X may have been adapted from ICU4C and/or ICU4J.
ICU 1.8.1 to ICU 57.1 © 1995-2016 International Business Machines Corporation and others.
```

### SIL Open Font License 1.1 (Geist, Geist Mono)

```text
Copyright 2026 The Geist Project Authors (https://github.com/vercel/geist-font)

This Font Software is licensed under the SIL Open Font License, Version 1.1.
This license is copied below, and is also available with a FAQ at:
https://openfontlicense.org


-----------------------------------------------------------
SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007
-----------------------------------------------------------

PREAMBLE
The goals of the Open Font License (OFL) are to stimulate worldwide
development of collaborative font projects, to support the font creation
efforts of academic and linguistic communities, and to provide a free and
open framework in which fonts may be shared and improved in partnership
with others.

The OFL allows the licensed fonts to be used, studied, modified and
redistributed freely as long as they are not sold by themselves. The
fonts, including any derivative works, can be bundled, embedded, 
redistributed and/or sold with any software provided that any reserved
names are not used by derivative works. The fonts and derivatives,
however, cannot be released under any other type of license. The
requirement for fonts to remain under this license does not apply
to any document created using the fonts or their derivatives.

DEFINITIONS
"Font Software" refers to the set of files released by the Copyright
Holder(s) under this license and clearly marked as such. This may
include source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the
copyright statement(s).

"Original Version" refers to the collection of Font Software components as
distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting,
or substituting -- in part or in whole -- any of the components of the
Original Version, by changing formats or by porting the Font Software to a
new environment.

"Author" refers to any designer, engineer, programmer, technical
writer or other person who contributed to the Font Software.

PERMISSION & CONDITIONS
Permission is hereby granted, free of charge, to any person obtaining
a copy of the Font Software, to use, study, copy, merge, embed, modify,
redistribute, and sell modified and unmodified copies of the Font
Software, subject to the following conditions:

1) Neither the Font Software nor any of its individual components,
in Original or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled,
redistributed and/or sold with any software, provided that each copy
contains the above copyright notice and this license. These can be
included either as stand-alone text files, human-readable headers or
in the appropriate machine-readable metadata fields within text or
binary files as long as those fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font
Name(s) unless explicit written permission is granted by the corresponding
Copyright Holder. This restriction only applies to the primary font name as
presented to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font
Software shall not be used to promote, endorse or advertise any
Modified Version, except to acknowledge the contribution(s) of the
Copyright Holder(s) and the Author(s) or with their explicit written
permission.

5) The Font Software, modified or unmodified, in part or in whole,
must be distributed entirely under this license, and must not be
distributed under any other license. The requirement for fonts to
remain under this license does not apply to any document created
using the Font Software.

TERMINATION
This license becomes null and void if any of the above conditions are
not met.

DISCLAIMER
THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE
COPYRIGHT HOLDER BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL
DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM
OTHER DEALINGS IN THE FONT SOFTWARE.
```
