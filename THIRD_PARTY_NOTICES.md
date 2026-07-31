# Third-party notices

ikashita is distributed under the MIT License; see [`LICENSE`](LICENSE).
This file records the registry dependencies in the committed `Cargo.lock` for
the standalone resource, CSV, and HTTP increments. Versions and license
expressions below were obtained from Cargo package metadata after lockfile
resolution on 2026-08-01. The lockfile remains authoritative for releases.

All dependencies listed here use permissive licenses or a license option
compatible with distribution under the ikashita MIT license. When distributing
compiled binaries or source packages, retain the license and notice files
shipped by each dependency. In particular, Apache-licensed packages require
retaining their Apache-2.0 notice and license text; the standard texts are
available at <https://www.apache.org/licenses/LICENSE-2.0> and
<https://opensource.org/license/mit>.

## Direct dependencies

| Crate | Locked version | License expression | Used by |
| --- | ---: | --- | --- |
| axum | 0.8.9 | MIT | ikashita-server |
| clap | 4.6.4 | MIT OR Apache-2.0 | ikashita-cli |
| csv | 1.4.0 | Unlicense/MIT | ikashita-csv |
| kdl | 6.5.0 | Apache-2.0 | ikashita-spec |
| serde | 1.0.229 | MIT OR Apache-2.0 | ikashita-resource |
| serde_json | 1.0.151 | MIT OR Apache-2.0 | resource/csv/server |
| serde_urlencoded | 0.7.1 | MIT/Apache-2.0 | ikashita-resource |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | CSV tests only |
| toml | 0.9.12+spec-1.1.0 | MIT OR Apache-2.0 | ikashita-cli |
| tokio | 1.53.1 | MIT | ikashita-server |
| tower | 0.5.3 | MIT | server tests only |

## Transitive dependencies

The following packages are also present in the lockfile. Packages with an
`OR` expression are used under the permissive MIT option where a choice is
needed. `r-efi` is only relevant to non-host targets pulled in by getrandom;
its LGPL option is not used by the host build, but is recorded here because it
is present in the lockfile.

| Crate | Locked version | License expression |
| --- | ---: | --- |
| anstyle | 1.0.14 | MIT OR Apache-2.0 |
| anstyle-parse | 1.0.0 | MIT OR Apache-2.0 |
| anstyle-query | 1.1.5 | MIT OR Apache-2.0 |
| anstyle-wincon | 3.0.11 | MIT OR Apache-2.0 |
| anstream | 1.0.0 | MIT OR Apache-2.0 |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT |
| axum-core | 0.5.6 | MIT |
| bitflags | 2.13.1 | MIT OR Apache-2.0 |
| bytes | 1.12.1 | MIT |
| cfg-if | 1.0.4 | MIT OR Apache-2.0 |
| clap_builder | 4.6.2 | MIT OR Apache-2.0 |
| clap_derive | 4.6.4 | MIT OR Apache-2.0 |
| clap_lex | 1.1.0 | MIT OR Apache-2.0 |
| colorchoice | 1.0.5 | MIT OR Apache-2.0 |
| csv-core | 0.1.13 | Unlicense/MIT |
| equivalent | 1.0.2 | Apache-2.0 OR MIT |
| errno | 0.3.14 | MIT OR Apache-2.0 |
| fastrand | 2.5.0 | Apache-2.0 OR MIT |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 |
| futures-channel | 0.3.33 | MIT OR Apache-2.0 |
| futures-core | 0.3.33 | MIT OR Apache-2.0 |
| futures-task | 0.3.33 | MIT OR Apache-2.0 |
| futures-util | 0.3.33 | MIT OR Apache-2.0 |
| getrandom | 0.4.3 | MIT OR Apache-2.0 |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 |
| heck | 0.5.0 | MIT OR Apache-2.0 |
| indexmap | 2.14.0 | Apache-2.0 OR MIT |
| http | 1.5.0 | MIT OR Apache-2.0 |
| http-body | 1.1.0 | MIT |
| http-body-util | 0.1.4 | MIT |
| httparse | 1.10.1 | MIT OR Apache-2.0 |
| httpdate | 1.0.3 | MIT OR Apache-2.0 |
| hyper | 1.11.0 | MIT |
| hyper-util | 0.1.20 | MIT |
| is_terminal_polyfill | 1.70.2 | MIT OR Apache-2.0 |
| itoa | 1.0.18 | MIT OR Apache-2.0 |
| libc | 0.2.189 | MIT OR Apache-2.0 |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| log | 0.4.33 | MIT OR Apache-2.0 |
| matchit | 0.8.4 | MIT AND BSD-3-Clause |
| memchr | 2.8.3 | Unlicense OR MIT |
| mime | 0.3.17 | MIT OR Apache-2.0 |
| mio | 1.2.2 | MIT |
| once_cell | 1.21.4 | MIT OR Apache-2.0 |
| once_cell_polyfill | 1.70.2 | MIT OR Apache-2.0 |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| r-efi | 6.0.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT OR LGPL-2.1-or-later |
| rustix | 1.1.4 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 |
| serde_path_to_error | 0.1.20 | MIT OR Apache-2.0 |
| slab | 0.4.12 | MIT |
| smallvec | 1.15.2 | MIT OR Apache-2.0 |
| socket2 | 0.6.5 | MIT OR Apache-2.0 |
| syn | 3.0.3 | MIT OR Apache-2.0 |
| sync_wrapper | 1.0.2 | Apache-2.0 |
| serde_spanned | 1.1.1 | MIT OR Apache-2.0 |
| strsim | 0.11.1 | MIT |
| toml_datetime | 0.7.5+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_parser | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_writer | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 |
| tokio-macros | 2.7.2 | MIT |
| tower-layer | 0.3.3 | MIT |
| tower-service | 0.3.3 | MIT |
| tracing | 0.1.44 | MIT |
| tracing-core | 0.1.36 | MIT |
| unicode-ident | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| wasi | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| windows-link | 0.2.1 | MIT OR Apache-2.0 |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 |
| winnow | 0.6.24, 0.7.15 | MIT |
| zmij | 1.0.23 | MIT |
| utf8parse | 0.2.2 | Apache-2.0 OR MIT |

Every package and exact version is individually recorded in `Cargo.lock`. No
dependency adds a project-specific attribution or runtime asset beyond the
license/notice files shipped with its crate. Dependency changes must update
this file and pass the repository license policy before release.
