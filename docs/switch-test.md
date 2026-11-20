# Switch test pass

A checklist for testing SlyNX on a Switch. The packages come from `tools/switch-test-package.sh --package`, which
builds them from your own discs into `~/slynx-packaged/` and prints the exact source and destination of every file.

## Copy

Copy each file on its own to the path the script prints, replacing the old file (with DBI, for example). Do not drop
whole folders: the overwrite prompt stalls.

| File (in `~/slynx-packaged/`) | Goes to |
|---|---|
| `sly1/switch/SlyNX/slynx-sly1.nro` | `sdmc:/switch/SlyNX/slynx-sly1.nro` |
| `sly2/switch/SlyNX/slynx-sly2.nro` | `sdmc:/switch/SlyNX/slynx-sly2.nro` |
| `sly3/switch/SlyNX/slynx-sly3.nro` | `sdmc:/switch/SlyNX/slynx-sly3.nro` |
| `viewer/switch/SlyNX/slynx-sly1.nro` | `sdmc:/switch/SlyNX/slynx-viewer.nro` |
| `smoke/switch/SlyNX/slynx-sly1.nro` | `sdmc:/switch/SlyNX/slynx-smoke.nro` |
| `video-bench/switch/SlyNX/slynx-sly1.nro` | `sdmc:/switch/SlyNX/slynx-video-bench.nro` |
| `sd/switch/SlyNX/user/video-bench.cfg` | `sdmc:/switch/SlyNX/user/video-bench.cfg` |

Each `.nro` carries its disc inside it; no disc image goes on the SD card. Do not put a `sly1.iso`, `sly2.iso` or
`sly3.iso` in `sdmc:/switch/SlyNX/`: the programs would read it instead of their own.

## Launch

Hold **R** while starting any installed game to open the Homebrew Menu with full memory (the Album is not enough),
then pick the program. Run the smoke and the video bench first: they need no input.

## Logs

Everything is written to `sdmc:/switch/SlyNX/user/`. Bring back:

- `smoke.log`: the smoke run (one line per stage, ends with `smoke: passed`, `failed` or `stopped`);
- `video-bench.log`: the movie decode times (default clock, then with the CPU boost);
- `perf.log`: the performance numbers from the games, written every 5 s while the overlay is on;
- any new file in `sdmc:/atmosphere/crash_reports/` after a crash.

Copy the logs off before relaunching a program: each run rewrites its own.

**Performance overlay**: in any of the three games, hold **L + R**, then press **ZL**. It shows the frame time and
its parts on screen and writes `perf.log`. Turn it on in each game before playing.

## What to look for

| Program | Check |
|---|---|
| Smoke | Runs on its own for about 2.5 minutes (title, Paris, the Hideout, each episode's first level) and exits. `smoke.log` ends with `smoke: passed`. **+** stops it early. |
| Video bench | Runs on its own, twice (the second time with the CPU boost), and exits. **+** stops it. |
| Sly 1 | Title, new game, Splash into Paris. Movies and dialogue play with sound and can be skipped. Run, jump (**B**), attack (**Y**). Save, quit to HOME, relaunch, load. 60 fps in the overlay. |
| Sly 2 | The nightclub hub loads with the gang. **D-pad** left / up / right switch Sly, Murray, Bentley. Walk into a job; watch subtitles and voice. Frame time in the overlay. |
| Sly 3 | Starts in Bentley's apartment (`v_apt`). The crew moves and switches on the d-pad. Frame time in the overlay. |
| Viewer | Browse worlds of Sly 1 (and Sly 2 if it lists them); each world draws without a crash. |
| All | Docked and handheld; HOME and back; sleep and wake. |

## When something breaks

Note what you were doing, docked or handheld, and the time. Bring the logs above and the crash report.
