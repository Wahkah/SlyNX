---
id: sly1-clock-and-loop
title: Sly 1 clocks, time scale and frame loop
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46 worlds
    used_for: key times in 1/60 s units; real-clock animations (option 189, 601 segments)
  - kind: observation
    what: the game as played
    used_for: pause behaviour, slow-motion and speed-up power-ups, 60 frames per second on NTSC
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files clock, main, update, ui, steppower, jsg, mgv, transition
    used_for: the timer base, the frame-step clamp, the time-scale factors and the update order
date: 2026-11-13
---

# Clocks, time scale and frame loop

Tags: [D] disc data, [O] observable, [R] reference behaviour to confirm, [I] inferred.

## 1. Measuring a frame

- The console's timer counts at 294,912,000 ticks per second; the frame step is (ticks since the previous frame) /
  294,912,000 seconds. [R]
- The measured step is clamped to **[1/60 s, 1/30 s]**: a fast frame still advances 1/60 s, a slow frame never
  advances more than 1/30 s (0.0333333 s). [R] The game therefore slows down rather than skips when it drops below
  30 frames per second. [I]
- The port runs a fixed 1/60 s step on the Switch and the PC when the frame rate holds; it must apply the same upper
  clamp when it falls behind. [I]

## 2. Two clocks

| Clock | Advances by | Used by |
|---|---|---|
| game clock | frame step × enabled × debug scale × power-up scale × rate | everything that plays: objects, physics, animations, scripts' scheduled calls, game time t |
| real clock | the clamped frame step; when the game step is at least 1/60 s it is the game step instead | menus, the pause overlay, faders, camera input, animations and objects flagged "real clock" |

- **Game time t** is the sum of game steps since the world started. Script operation 96 returns it (scripts.md 7).
  [R]
- **Enabled** is 0 while the clock rate is 0 (paused), else 1. [R]

## 3. Time-scale factors

All multiply the game step. [R] unless marked.

| Factor | Values | Set by |
|---|---|---|
| rate | 0 to 1 | the pause screen: it eases the rate between 0 (paused) and 1; while it does, the music volume follows 0.6 + 0.4 × rate. Jump guides set their own rate per step, then restore 1. Level transitions restore 1 |
| power-up | 0.25 (slow-motion power-up), 1 + u × 0.5 (speed-up power-up, u = its strength 0 to 1), 0.2 (a vehicle minigame's damage flash), else 1 | power-ups and minigames; reset to 1 when a world loads [R] [O for the effects] |
| debug | 1 | development builds only |

## 4. Order of one frame

The list in gameplay-overview.md section 2 is the frame order. Facts that matter for implementing it: [R]

- The controller is read once per frame, before anything updates.
- The player-controlled object decides its move **before** the other objects update and before physics, using
  last frame's positions.
- Objects update parent before child; the order of siblings is the order they were added to the world (stream order
  for loaded objects).
- Animation playbacks advance **after** object updates and **before** physics; physics then reacts to the new
  animated positions.
- Script scheduled calls run after physics, then queued callbacks.
- The camera updates after all of that, every frame, including paused frames (it uses the real clock for input).
- Finished playbacks are removed at the end of the frame, so events raised during the frame can still see them.

## 5. Test vectors

| Situation | Measured interval | Game step | Real step |
|---|---|---|---|
| steady 60 fps | 1/60 s | 0.0166667 s | 0.0166667 s |
| 120 fps host (port at uncapped rate) | 1/120 s | 0.0166667 s (clamped up) | 0.0166667 s |
| a 50 ms hitch | 0.05 s | 0.0333333 s (clamped down) | 0.0333333 s |
| paused | any | 0 | the clamped step |
| slow-motion power-up at 60 fps | 1/60 s | 0.0041667 s | 0.0166667 s (the game step is below 1/60) |

## 6. Open questions

1. Whether the original's lower clamp is exactly 1/60 s in the PAL release (50 Hz) or 1/50 s. The USA release is
   the target; PAL is out of scope until a PAL spec exists.
2. The pause screen's easing curve for the rate (belongs to the menus spec).
