# Remaining Issues Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Project note:** the maintainer executes these tasks himself. Tasks are ordered by dependency; each ends with an observable verification (a green pipeline, a played-back file, a rendered README), not a unit test — none of this work adds testable Rust code.

**Goal:** Close out every open issue: #9 (per-output overlays — final hardware verification), #8 (CI green on both platforms), #7 (demo recording in the README), #6 (v0.1.0 release + AUR package).

**Architecture:** The per-output-overlays feature is already merged to `master` (PR #11); what remains is verification, documentation, and distribution. Order matters: hardware verification truths the CHANGELOG before the release is cut, CI must be green on both platforms before tagging, the demo lands in the README so the tagged tarball ships complete docs, and the AUR PKGBUILD points at the tag — so the release must exist first.

**Tech Stack:** git/gh CLI, GitLab CI, wf-recorder + ffmpeg (demo), makepkg/updpkgsums/.SRCINFO (AUR).

## Global Constraints

- **No AI attribution anywhere** — commits, issue comments, release notes, PKGBUILD. Plain human-authored messages in the existing conventional-commit style (`feat:`, `docs:`, `build:`, `fix:`).
- **Pre-push gate:** the global `rust-sec-ci` hook runs fmt/clippy/audit on every `git push` (GitHub, GitLab, and AUR remotes alike). A rejection means fix the finding — never disable `core.hooksPath`.
- **Thermal cap:** launch every heavy non-cargo process through `hotrun` (the ffmpeg demo transcodes, `makepkg`). `cargo` joins `buildwork.slice` automatically via the PATH shim.
- **Line numbers** below are anchored to current `master`. Tasks shift lines within the same file; the anchor text quoted next to each edit is authoritative.

## Issue → Task map

| Issue | Remaining work | Tasks |
|---|---|---|
| #9 per-output overlays | physical dual-monitor verification, CHANGELOG truthing, close | 1, 2 |
| #8 CI on both platforms | GitLab pipeline verification, badge check | 3 |
| #7 demo recording | record, optimise, embed in README | 4 |
| #12 unstoppable recording | notify how to stop when the selection hides the Stop bar | 4b |
| #6 packaging + release | CHANGELOG cut, tag v0.1.0, GitHub release, AUR PKGBUILD, README install docs | 5, 6 |

---

### Task 1: Sync master and clean up the merged branch

The feature branch `per-output-overlays` is merged (PR #11); the local checkout still sits on it.

**Files:** none (git housekeeping).

- [x] **Step 1: Switch to master and pull**

```bash
git checkout master
git pull origin master
```

Expected: fast-forward to the PR #11 merge commit (`00af2c2`'s successor or later).

- [x] **Step 2: Delete the merged branch locally and on origin**

```bash
git branch -d per-output-overlays
git push origin --delete per-output-overlays
```

Expected: `-d` succeeds (proof the branch is fully merged — if git refuses, stop and check why rather than forcing with `-D`). The remote delete may already be a no-op if GitHub auto-deleted it.

- [x] **Step 3: Clear the anyhow advisory blocking all pushes** *(done: `bface4a`)*

Discovered on first push: the pre-push gate's `deny` stage rejects `anyhow 1.0.102` — RUSTSEC-2026-0190, unsoundness in `Error::downcast_mut()` (UB when downcasting a context-wrapped error mutably; lanner never calls `downcast_mut`, but the gate's advisory policy is deny-unsound regardless). Fixed upstream in 1.0.103; `Cargo.toml`'s `"1.0.102"` caret requirement already admits it, so only the lockfile moves:

```bash
cargo update -p anyhow
cargo test
git add Cargo.lock
git commit -m "build: bump anyhow to clear RUSTSEC-2026-0190"
```

Expected: `Updating anyhow v1.0.102 -> v1.0.103` (or later), tests green, and subsequent pushes pass the gate.

- [x] **Step 4: Commit this plan and push** *(done: `5240c2a`; first GitLab push hit a transient SSH broken pipe — retry succeeded)*

```bash
git add docs/superpowers/plans/2026-07-16-remaining-issues.md
git commit -m "docs: remaining-issues implementation plan"
git push origin master
git push gitlab master
```

Note: local `master` tracks `gitlab/master`, so the prompt shows `⇡N` until both remotes have the commits — push both.

- [x] **Step 5: Retry the remote branch delete, confirm a clean baseline**

```bash
git push origin --delete per-output-overlays
git status
```

Expected: delete succeeds now the gate passes (it runs on every push, even ref deletions); tree clean.

---

### Task 2: Close out issue #9 — physical dual-monitor verification

The CHANGELOG currently states the per-output path is "not yet confirmed on physical dual-monitor hardware" (CHANGELOG.md:46-53). This task makes that sentence true-and-done, then closes the issue.

**Files:**
- Modify: `CHANGELOG.md:50-53` (the per-output overlays bullet's verification sentence)
- Update: issue #9 (comment + close)

**Produces:** a truthful CHANGELOG entry that Task 5 will ship in the release.

- [ ] **Step 1: Run the physical verification checklist**

Connect the second monitor, then work through each line. Every line must pass:

1. `hyprctl monitors` — both outputs listed; note their names and layout origins.
2. Launch `lanner` — **both** monitors dim; the control bar appears on the monitor the pointer is on.
3. Drag a rectangle on monitor A — hole + border appear there; monitor B stays fully dimmed.
4. Without releasing anything else, drag on monitor B — the selection moves to B (last-wins, one selection total; A goes back to plain dim).
5. Press Enter with the selection on B — recording starts on B: B drops to border-only + Stop bar, **A undims completely and is clickable** (open a window on A to prove input passthrough).
6. Press Stop — lanner exits, the transcode notification fires, and playback of the file shows monitor B's content, the drawn region only, at B's native resolution.
7. Repeat steps 3-6 recording monitor A instead.
8. If either monitor runs scaled (HiDPI), confirm the captured region matches what was drawn (the rig already verified scale 2.0; this is belt-and-braces on real hardware).

If any line fails: stop here, file the failure as a comment on issue #9, and debug before touching the CHANGELOG — the remaining tasks assume the feature is done.

- [ ] **Step 2: Truth the CHANGELOG**

In `CHANGELOG.md`, the per-output bullet ends with (lines 50-53):

```
  output. Verified so far: an overlay maps per output (both outputs on a headless
  rig), and single-output capture works; the cross-monitor draw, record, and undim
  path is unit-tested and rig-verified for the geometry but not yet confirmed on
  physical dual-monitor hardware (issue #9).
```

Replace those lines so the bullet ends:

```
  output. Unit-tested, rig-verified on a headless dual-output layout, and
  confirmed on physical dual-monitor hardware (issue #9).
```

- [ ] **Step 3: Commit and push**

```bash
git add CHANGELOG.md
git commit -m "docs: per-output overlays verified on physical dual-monitor hardware"
git push origin master
```

Expected: pre-push gate passes; the CI run on master goes green.

- [ ] **Step 4: Comment and close issue #9**

The two documented out-of-scope limits (spanning selections, monitor hotplug) already live in the README's Limitations section, so close outright — open new narrow issues later only if you actually plan to build them.

```bash
gh issue comment 9 --body "Per-output overlays landed in #11: every monitor dims, the region can be drawn on any output, and that output is recorded while the others undim and pass input through. Now verified on physical dual-monitor hardware in addition to the headless rig (origin translation and HiDPI scale included).

Remaining limits — a region spanning two outputs and live monitor hotplug — are out of scope (wf-recorder records a single output) and documented in the README's Limitations section. Closing."
gh issue close 9
```

Expected: issue #9 shows CLOSED.

---

### Task 3: Issue #8 — CI green on GitHub and GitLab

GitHub CI is already green (the last five runs on master all passed, including the gtk4-layer-shell build-from-source step — the issue's first checkbox is already satisfied by `.github/workflows/ci.yml`). What's unverified is the GitLab side and the badge.

**Files:**
- Modify: `README.md:5` (optionally add a GitLab pipeline badge next to the GitHub one)
- Update: issue #8 (close)

- [x] **Step 1: Push master to GitLab** *(done during Task 1 Step 4)*

```bash
git push gitlab master
```

Expected: the pre-push gate runs a second time (it fires per-push, per-remote) and passes.

- [x] **Step 2: Watch the GitLab pipeline** *(pipeline #6 on master: success, 2026-07-16)*

Open <https://gitlab.com/tidynest/lanner/-/pipelines> (or `glab ci list` if you have glab). The `check` job builds gtk4-layer-shell from source exactly like the GitHub job, so expect ~5-10 min on a shared runner.

Expected: pipeline **passed**.

Failure modes, in likelihood order:
- *Pipeline never starts:* shared runners are disabled for the project — enable under Settings → CI/CD → Runners.
- *apt or meson step fails:* the `rust:latest` image changed its Debian base; read the job log — the fix is usually one extra `apt-get install` package in `.gitlab-ci.yml`'s `before_script`.
- *clippy/fmt fails:* would mean master drifted — it can't, the same commands are green on GitHub; re-check you pushed the same SHA.

- [x] **Step 3: Verify the badge(s) render** *(badge SVGs fetched: GitHub "passing", GitLab "passed"; optional GitLab badge in README still open below)*

Open <https://github.com/tidynest/lanner> — the CI badge at the top of the README must show **passing**, not "no status".

Optional (recommended for platform parity): add a GitLab badge. In `README.md`, after line 5 (the GitHub CI badge), insert:

```markdown
[![GitLab CI](https://gitlab.com/tidynest/lanner/badges/master/pipeline.svg)](https://gitlab.com/tidynest/lanner/-/pipelines)
```

```bash
git add README.md
git commit -m "docs: add GitLab pipeline badge"
git push origin master && git push gitlab master
```

- [x] **Step 4: Close issue #8** *(closed 2026-07-16; GitLab badge added in `f07c6b1`)*

```bash
gh issue close 8 --comment "Green on both platforms: GitHub Actions builds gtk4-layer-shell from source and passes fmt/clippy/test/build; the GitLab check job passes with the same steps. Badges render in the README."
```

Expected: issue #8 shows CLOSED.

---

### Task 4: Issue #7 — record the demo and embed it in the README

One honest constraint up front: **lanner cannot film its own UI** — by design the dim, border, and bar sit outside the captured geometry ("never films its own UI"). So the demo is filmed *of* lanner *by* plain `wf-recorder` capturing the whole output. The flow shown is still 100% lanner.

**Files:**
- Create: `docs/demo.webp` (or `docs/demo.gif` as fallback)
- Modify: `README.md:15-19` (the Demo placeholder section)
- Update: issue #7 (close)

**Consumes:** the dual-monitor setup from Task 2 makes this easy (drive from B, film A), but a single monitor works too — the launching terminal is just visible in frame for a moment.

- [ ] **Step 1: Plan the ~20-second shot**

Script to perform on the filmed monitor:
1. Press the lanner keybind (Super+Shift+R) — screen dims, bar appears.
2. Pick a format on the bar (one click, shows the pickers exist).
3. Drag a rectangle around a live window (terminal running something, or a video).
4. Press Enter — dim lifts to the border; interact inside the region for ~5 s (type, scroll) to show passthrough.
5. Click Stop — desktop returns, the notification pops.

- [ ] **Step 2: Capture with wf-recorder**

From a terminal on the *other* monitor (or the same one, minimized after starting):

```bash
hyprctl monitors | grep Monitor          # get the output name to film, e.g. eDP-1
wf-recorder -o eDP-1 -f ~/Videos/lanner-demo.mkv
# ...perform the Step 1 script on eDP-1, then back here:
# Ctrl-C  (SIGINT finalises the MKV cleanly)
```

Note: during lanner's *selection* phase every monitor dims — including the one your terminal is on. That's fine; you don't need the terminal until after Stop. Don't click on the terminal monitor while the overlay is up, or you'll move the selection there.

Expected: `~/Videos/lanner-demo.mkv` plays back showing the full flow.

- [ ] **Step 3: Trim and transcode to animated WebP (primary) and GIF (fallback)**

Trim dead time first if needed (adjust `-ss`/`-t`):

```bash
hotrun ffmpeg -ss 1 -t 20 -i ~/Videos/lanner-demo.mkv -c copy /tmp/demo-cut.mkv
```

Animated WebP — roughly 10× smaller than GIF, renders in GitHub and GitLab READMEs:

```bash
hotrun ffmpeg -i /tmp/demo-cut.mkv -vf "fps=15,scale=960:-1:flags=lanczos" \
  -c:v libwebp -q:v 55 -compression_level 6 -loop 0 docs/demo.webp
```

GIF fallback, only if the WebP doesn't animate where you need it:

```bash
hotrun ffmpeg -i /tmp/demo-cut.mkv -vf "fps=12,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128[p];[b][p]paletteuse=dither=bayer:bayer_scale=4" docs/demo.gif
```

Check the size — `ls -lh docs/demo.*` — target **WebP ≤ 3 MB** (GIF ≤ 8 MB). Too big? Knobs in order of visual cost: shorten the clip, `scale=800:-1`, `fps=12` (WebP) / `fps=10` (GIF), `max_colors=96`.

Expected: the file plays in an image viewer (`imv docs/demo.webp`) and is within budget.

- [ ] **Step 4: Replace the README placeholder**

`README.md` lines 15-19 currently read:

```markdown
## Demo

A short clip recorded with lanner itself will live here (tracked as the M8
demo).
```

Replace with:

```markdown
## Demo

![lanner: draw a region, record it, keep using the desktop](docs/demo.webp)

The spotlight dim, the rubber-band selection, the border-only recording phase,
and the finish notification — filmed with `wf-recorder`, since lanner keeps its
own UI out of everything it records.
```

- [ ] **Step 5: Commit, push, verify on GitHub**

```bash
git add docs/demo.webp README.md
git commit -m "docs: record and embed the demo"
git push origin master && git push gitlab master
```

Open <https://github.com/tidynest/lanner> and confirm the demo **animates** in the rendered README. If it renders as a static frame, redo Step 3's GIF variant, swap the README reference to `docs/demo.gif`, and re-commit.

- [ ] **Step 6: Close issue #7**

```bash
gh issue close 7 --comment "Demo recorded (select → record with passthrough → stop → notification), optimised to an animated WebP under 3 MB, and embedded in the README. Filmed with wf-recorder because lanner deliberately keeps its own UI out of its captures."
```

Expected: issue #7 shows CLOSED.

---

### Task 4b: Tell the user how to stop when the Stop bar cannot be placed

*Issue #12 — filed during execution, found by hitting it live:* a large selection leaves no dim band big enough for the ~220×44 bar, so `place_off_hole` returns `None` and `app.rs` hides the bar entirely. Recording has already set `KeyboardMode::None` (passthrough), so Esc is gone too, and layer-shell surfaces ignore the compositor's close-window bind. The only exit is a second keybind press — and the only place lanner says so is `tracing::info!` on stdout, which does not exist when launched from a keybind. That is the one and only situation where this happens, so the message never reaches anyone.

Fix: a desktop notification at the moment the bar is hidden. Must land before Task 5 so v0.1.0 ships with it.

**Files:**
- Modify: `src/app.rs:188-192` (the `None` arm of the `place_off_hole` match)
- Modify: `src/app.rs` (add the `notify` helper next to `stop_and_quit`, around line 292)
- Modify: `README.md:104-105` (the full-screen-selection note)
- Modify: `CHANGELOG.md` (`[Unreleased]` → `### Fixed`)

**Interfaces:**
- Produces: `fn notify(body: &str)` — private to `app.rs`, fire-and-forget, no return.

- [ ] **Step 1: Add the `notify` helper**

In `src/app.rs`, immediately above `fn stop_and_quit` (currently line 292):

```rust
/// Best-effort desktop notification. `notify-send` is an optional dependency, so
/// a missing binary or a failed spawn is silently fine. Critical urgency is
/// deliberate: per the freedesktop spec such notifications never auto-expire, and
/// this one has to still be on screen when the user starts hunting for a way out.
fn notify(body: &str) {
    // ponytail: child is never reaped, so it lingers as a zombie until lanner
    // exits. notify-send lives for milliseconds and lanner is short-lived, so
    // that is one defunct entry at worst. Reap it if lanner ever daemonises.
    let _ = std::process::Command::new("notify-send")
        .args(["--app-name=lanner", "--urgency=critical", "lanner", body])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
```

- [ ] **Step 2: Call it where the bar is hidden**

In `src/app.rs`, the `None` arm currently reads:

```rust
                        None => {
                            bar.set_visible(false);
                            tracing::info!("full-screen capture: stop with the keybind");
                            (0, 0, 0, 0)
                        }
```

Replace it with:

```rust
                        None => {
                            bar.set_visible(false);
                            notify(
                                "Recording with no room for the Stop bar. \
                                 Press your lanner keybind again to stop.",
                            );
                            tracing::info!("no room for the stop bar: stop with the keybind");
                            (0, 0, 0, 0)
                        }
```

The `tracing::info!` stays — it is still the right record for a terminal-launched run; the notification is what reaches a keybind-launched one.

- [ ] **Step 3: Build and verify live**

```bash
cargo build --release
cp target/release/lanner ~/.local/bin/     # or ~/.cache/cargo-target/release/lanner
```

No new unit test: `notify` is a fire-and-forget spawn with no logic, and the `None` branch that triggers it is already covered by `bar_dodges_hole_or_reports_no_room` in `app.rs`.

Verify by hand — this is the bug's own reproduction:

1. Press the lanner keybind, drag a rectangle covering nearly the whole monitor (within ~70 px of the top and bottom edges, ~250 px of the left and right).
2. Press Enter. The Stop bar must be absent **and** a notification must appear reading "Recording with no room for the Stop bar…".
3. Confirm the notification does not auto-dismiss after a few seconds.
4. Press the lanner keybind again. Recording stops, the transcode notification follows, and a final file appears in `~/Videos` beside the MKV — proof the clean stop path ran rather than a kill.

- [ ] **Step 4: Update the docs**

`README.md` lines 104-105 currently read:

```markdown
At a full-screen selection there is no room for the on-screen controls, so use
the keybind to stop.
```

Replace with:

```markdown
When the selection leaves no room for the on-screen controls, the bar is hidden
so it is never filmed, and lanner sends a desktop notification telling you to
press the keybind again to stop.
```

In `CHANGELOG.md`, under `[Unreleased]`, add a `### Fixed` entry:

```
- A selection large enough to leave no room for the control bar hid the Stop
  button, while recording had already released the keyboard for passthrough, so
  neither the button nor Esc could stop it. The instruction to use the keybind
  went only to stdout, which does not exist when lanner is launched from a
  keybind. lanner now sends a desktop notification instead.
```

- [ ] **Step 5: Commit and push**

```bash
git add src/app.rs README.md CHANGELOG.md
git commit -m "fix: notify how to stop when the control bar cannot be placed (#12)"
git push origin master && git push gitlab master
gh issue close 12 --comment "Fixed: lanner now sends a critical-urgency desktop notification when the control bar cannot be placed clear of the selection, pointing at the keybind as the way to stop. Verified by hand with a near-fullscreen selection."
```

---

### Task 5: Issue #6, part 1 — cut v0.1.0 and publish the GitHub release

Everything above must be done first: the release tarball snapshots the README (badges + demo) and the truthed CHANGELOG.

**Files:**
- Modify: `CHANGELOG.md:8` (cut `[Unreleased]` into `[0.1.0]`, add link refs)
- Create: git tag `v0.1.0`, GitHub release

**Produces:** tag `v0.1.0` — Task 6's PKGBUILD `source=` URL points at it. `Cargo.toml` is already `version = "0.1.0"`, so no version bump commit is needed.

- [ ] **Step 1: Cut the CHANGELOG**

In `CHANGELOG.md`, replace line 8:

```markdown
## [Unreleased]
```

with:

```markdown
## [Unreleased]

## [0.1.0] - 2026-07-16
```

(use the actual release date) and append the Keep-a-Changelog link refs at the very bottom of the file:

```markdown
[Unreleased]: https://github.com/tidynest/lanner/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/tidynest/lanner/releases/tag/v0.1.0
```

- [ ] **Step 2: Commit the cut and wait for green CI**

```bash
git add CHANGELOG.md
git commit -m "docs: cut 0.1.0"
git push origin master && git push gitlab master
gh run watch
```

Expected: the master CI run completes green **before** you tag — never tag an unverified commit.

- [ ] **Step 3: Tag and push the tag to both remotes**

```bash
git tag -a v0.1.0 -m "lanner 0.1.0"
git push origin v0.1.0
git push gitlab v0.1.0
```

- [ ] **Step 4: Create the GitHub release with the CHANGELOG section as notes**

```bash
awk '/^## \[0\.1\.0\]/{f=1;next} /^## \[/{f=0} f' CHANGELOG.md > /tmp/relnotes.md
gh release create v0.1.0 --title "lanner 0.1.0" --notes-file /tmp/relnotes.md
```

Expected: <https://github.com/tidynest/lanner/releases/tag/v0.1.0> exists with the notes, and GitHub auto-attaches the source tarball (`Source code (tar.gz)`) — that satisfies the issue's "attach a source tarball" checkbox and is exactly the artifact the PKGBUILD downloads.

---

### Task 6: Issue #6, part 2 — AUR PKGBUILD, publish, README install docs

**Files:**
- Create: `packaging/aur/PKGBUILD` (reviewed, versioned copy in this repo)
- Create: AUR repo `ssh://aur@aur.archlinux.org/lanner.git` (PKGBUILD + .SRCINFO — the canonical copy)
- Modify: `README.md` (new Install section between Requirements and Build, after current line 66)
- Update: issue #6 (close)

**Consumes:** tag `v0.1.0` from Task 5 (the `source=` URL 404s without it).

- [ ] **Step 1: One-time AUR prerequisites**

An AUR account at <https://aur.archlinux.org> with your SSH public key added under My Account → SSH Public Key, and `pacman-contrib` installed (`sudo pacman -S --needed pacman-contrib` for `updpkgsums`).

- [ ] **Step 2: Write the PKGBUILD in this repo**

Create `packaging/aur/PKGBUILD`:

```bash
# Maintainer: Eric Jingryd <eric.jingryd@protonmail.com>
pkgname=lanner
pkgver=0.1.0
pkgrel=1
pkgdesc="Spotlight screen-region video recorder for wlroots Wayland compositors"
arch=('x86_64')
url="https://github.com/tidynest/lanner"
license=('GPL-3.0-or-later')
depends=('wf-recorder' 'ffmpeg' 'gtk4' 'gtk4-layer-shell')
optdepends=('libnotify: desktop notification when the transcode finishes'
            'wl-clipboard: copy the saved path to the clipboard')
makedepends=('cargo')
source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('SKIP')

prepare() {
  cd "$pkgname-$pkgver"
  export RUSTUP_TOOLCHAIN=stable
  cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
  cd "$pkgname-$pkgver"
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR=target
  cargo build --frozen --release
}

check() {
  cd "$pkgname-$pkgver"
  cargo test --frozen --release
}

package() {
  cd "$pkgname-$pkgver"
  install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
  install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
```

Why each piece: `--locked`/`--frozen` pin the committed `Cargo.lock` (Arch Rust package guidelines); `RUSTUP_TOOLCHAIN=stable` is a no-op on plain `rust` but keeps rustup users on stable; `check()` runs only the pure geometry/argv tests, so it needs no display; the four `depends` are runtime (`gtk4-layer-shell` is in [extra]); `notify-send`/`wl-copy` are genuinely optional in the code, hence `optdepends`.

- [ ] **Step 3: Fill the real checksum and generate .SRCINFO in an AUR clone**

```bash
git clone ssh://aur@aur.archlinux.org/lanner.git ~/aur-lanner
cd ~/aur-lanner
git switch -c master 2>/dev/null || true     # AUR accepts only 'master'
cp ~/RustroverProjects/lanner/packaging/aur/PKGBUILD .
updpkgsums                                   # replaces SKIP with the real sha256
makepkg --printsrcinfo > .SRCINFO
```

Expected: cloning the empty repo warns ("you appear to have cloned an empty repository") — that's how a new AUR package starts. `updpkgsums` downloads the tag tarball and rewrites `sha256sums`.

Copy the checksummed PKGBUILD back so the in-repo copy matches: `cp PKGBUILD ~/RustroverProjects/lanner/packaging/aur/PKGBUILD`.

- [ ] **Step 4: Test-build and install locally**

```bash
cd ~/aur-lanner
hotrun makepkg --cleanbuild --syncdeps --install
lanner   # smoke test: overlay appears; Esc to quit
```

Optional lint: `namcap PKGBUILD lanner-0.1.0-1-x86_64.pkg.tar.zst` (package `namcap`); warnings about unused sodeps from GTK are normal, errors are not.

Expected: package builds, installs to `/usr/bin/lanner`, and the installed binary runs.

- [ ] **Step 5: Publish to the AUR**

```bash
cd ~/aur-lanner
git add PKGBUILD .SRCINFO
git commit -m "lanner 0.1.0-1: initial release"
git push origin master
```

Expected: <https://aur.archlinux.org/packages/lanner> is live within a minute.

- [ ] **Step 6: Document the AUR install in the README and commit everything**

In `README.md`, insert between the Requirements section's closing code fence (currently line 66) and `## Build` (currently line 68):

````markdown
## Install

On Arch Linux, install [lanner from the AUR](https://aur.archlinux.org/packages/lanner):

```bash
paru -S lanner        # or: yay -S lanner
```

The runtime dependencies (`wf-recorder`, `ffmpeg`, `gtk4`, `gtk4-layer-shell`)
are pulled in automatically; building from source (below) is only needed off
Arch or for development.
````

Then in the main repo:

```bash
cd ~/RustroverProjects/lanner
git add packaging/aur/PKGBUILD README.md
git commit -m "build: AUR PKGBUILD and install docs"
git push origin master && git push gitlab master
```

- [ ] **Step 7: Close issue #6**

```bash
gh issue close 6 --comment "Shipped: v0.1.0 tagged with the GitHub release and source tarball, lanner published to the AUR (PKGBUILD mirrored in packaging/aur/), and the README documents the AUR install."
```

Expected: issue #6 shows CLOSED — zero open issues remain.

---

## Self-review notes

- Issue coverage: #9 hardware confirmation + close (Tasks 1-2, both checklist gaps), #8 all three checkboxes (Task 3: runner dep already solved in ci.yml, GitLab job verified Step 2, badge Step 3), #7 all three checkboxes (Task 4: capture Step 2, optimise Step 3, placeholder replaced Step 4), #6 all three checkboxes (Task 5 tag+tarball, Task 6 PKGBUILD + README docs).
- Ordering dependencies stated where they bind: CHANGELOG truthing (Task 2) and demo (Task 4) precede the tag (Task 5); the tag precedes the PKGBUILD source URL (Task 6).
- The one deviation from an issue's letter — the demo is filmed *by* wf-recorder, not *with* lanner itself — is a structural property of lanner (it excludes its own UI from captures) and is disclosed in the README text and the issue-close comment.
