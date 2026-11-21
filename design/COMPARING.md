# Comparing the app with the mock

The mock's screens are captured from `design/mock-v2/index.html` by `design/mock-v2/capture.mjs`, into
`design/mock-v2/captures/states/<theme>/<state-id>.webp`. The app's screens are captured by
`design/app-captures/capture.sh`, which runs `crates/ledgerly-app/examples/ui_snapshots.rs` on a fictional
company and writes lossless WebP files to `design/app-captures/<theme>/<state-id>.webp`. Both sides use the
same state ids (`design/mock-v2/states.json`) and the same themes (`light`, `dark`, `hc`), so a file
name identifies the same screen on both sides. Run `node design/mock-v2/capture.mjs` first when the mock
changes, then `design/app-captures/capture.sh` (add `--only <substring>` to capture a few entries).

The script then runs `design/compare.mjs`, which pairs every app capture with the mock capture of the same
theme and state id and writes `design/COMPARE.md`. Each pair has its own section with the mock and the app
image side by side, and an SSIM score from ffmpeg (1 is identical, lower is more different). The sections
run worst first, so the top of the file is the work to do. A table at the top lists every pair with its
score. Mock states that have no app capture yet are listed at the bottom, per theme; that list is the
coverage gap, not a failure.

Read `COMPARE.md` on GitHub or in any Markdown viewer that shows images; the paths in it are relative to
`design/`. A low score can come from a real visual difference or from a different window size or a moved
element, so look at the pair before changing the code. The score says where to look; it is not a pass mark.
