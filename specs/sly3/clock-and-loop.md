---
id: sly3-behaviour-clock-and-loop
title: Sly 3 clocks and frame loop - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: Sly 3 USA (SCUS-97464), executable SCUS_974.64 SHA-256 ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: the clock block's address and its readers [X]
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release (runs Sly 2's clock and update code)
    used_for: the statement that the clock rules are Sly 2's; marked [R]
reviewed_by: pending
date: 2026-11-19
---

# Sly 3 clocks and frame loop

Read [../sly2/clock-and-loop.md](../sly2/clock-and-loop.md): the frame measurement (step clamped to 1/60 … 1/30 s,
the first frame after a reset 1/30 s), the nine rate factors, activation, the update list and the frame order are
taken as unchanged [R]. Tags as in [guards.md](guards.md).

## 1. What the executable shows [X]

- The clock block lies at `0x0036BB88`. The float at `0x0036BB98` is the **game time t**. Several hundred code
  sites read it. Timers store it when a state starts and compare t minus that start with a limit: the bouncer's
  0.05 s ([alarms-sensors.md](alarms-sensors.md) 5) and the safe controller's state times
  ([pickpocket-and-pickups.md](pickpocket-and-pickups.md) 6.1).
- The float at `0x0036BB9C` is the **game step**: the power drain multiplies the drain rate by it
  ([gadgets.md](gadgets.md) 2) [X for the use; I for the name].
- The two gadget rate values of Sly 2 (0.25, 1.35) were not searched for, because Sly 3 has no slow-motion or
  speed-up gadget [O].

## 2. Open questions

1. Whether Sly 3 keeps Sly 2's nine rate factors and their users (the shop, the hacking games and the two-player
   modes are candidates).
2. The rest of the clock block (the real clock, the rate factors).
3. The frame-step clamp and the first-frame rule, confirmed in Sly 3's own code.
