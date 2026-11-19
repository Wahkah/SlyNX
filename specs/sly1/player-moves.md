---
id: sly1-player-moves
title: Sly 1 player character moves
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46
      worlds; executable SCUS_971.98 sha256 d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c
    used_for: >-
      move speeds computed from the root-motion channels of the player's animation segments (Paris), foot-plant
      markers, segment durations, the player's own options
  - kind: observation
    what: the game as played
    used_for: the list of moves and their controls
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files jt, jt.h, jump, jump.h, step, steprun, stephide, steppipe, steppower, stepzap, stepcane, blend, act
    used_for: >-
      jump rules and constants, gravity ratio, timers, input mapping, the lookup-list roles, damage selection, the
      marker interval used for the run speed; values with an executable address there were read as stated and
      re-read from the executable; values without one are [R] until located in the executable
date: 2026-11-14
---

# Player character moves

Tags: [D] disc data (including root-motion speeds measured from the animation data), [X] a constant read from the
executable at the given virtual address (file offset = address − 0x100000 + 0x1000; the port checks the
executable's SHA-256 in disc.md before reading and refuses on a mismatch; every [X] value here was read from the
USA executable), [O] observable, [R] reference behaviour, [I] inferred.
Units: game units (about 1 cm), seconds. Animation OIDs refer to the player's lookup list (animation.md 7).

## 1. Physical parameters

| Parameter | Value | Tag |
|---|---|---|
| gravity | −1750 units/s² × ratio 1.3 = **−2275 units/s²** | [R] |
| terminal fall | below −750·√1.3 = −855.1 units/s an extra 5·(−855.1 − v_z) per second is added to gravity, so the fall settles at **−1310.1 units/s** (physics.md 2) | [X] 0x263E78, 0x263E7C |
| air-steer speed | 455 units/s × √1.3 = 518.8 units/s | [X] 0x262E14 |
| air-steer factor after a jump from water | 0.7 | [X] 0x262E18 |
| air-steer factor after an edge jump | 0.6 | [X] 0x262E1C |
| hook catch radius | 15 units | [R] |
| charm flash duration after a hit | 2 s | [R] |

## 1.1 Where the origin is

The player's origin is not at his feet. His standing collision shape (OID 100, a child solid) reaches 55 units below
the origin [D]; the walking leg's base point is 35 units (option 709 default) below that (physics.md 5.1), so when
standing on the ground the origin is **90 units above the ground contact point** [D for 55, R for the rule that
the shape is used at the origin, without the child's own 15-unit placement]. Specs that give heights relative to
the player (jump height, ledge hang, camera focus) mean this origin.

## 2. Running

- The stick's direction sets the facing target; its deflection u (0 to 1) blends two locomotion segments: the
  slow one (OID 130) with weight 1 − u and the fast one (OID 131) with weight u. [R]
- Each segment's speed is its root-motion displacement divided by its duration. Measured on the disc (Paris): [D]

| Segment | Duration | Forward displacement | Speed |
|---|---:|---:|---:|
| OID 130 (slow) | 180 frames | 114.8 units | **38.3 units/s** |
| OID 131 (fast) | 32 frames | 340.5 units | **638.4 units/s** |

- Foot markers (label events, animation.md 5.3): left foot plant 286 and lift 287, right foot plant 288 and lift
  289. Segment 130: 288 at frame 0, 287 at 41, 286 at 90, 289 at 131. Segment 131: 288 at 8, 289 at 14, 286 at 23,
  287 at 30. [D]
- **Target speed at mid deflection.** Each segment has, per foot, two marker intervals: from the plant to the lift
  and from the lift to the next plant (wrapping round the segment's end). The intervals that set the run speed
  are the **left foot's plant-to-lift** intervals: 286 → 287, which is **131 frames** in segment 130
  (90 → 180 → 41) and **7 frames** in segment 131 (23 → 30). [D] The blended animation's interval is the
  weighted sum of the members' intervals, and the character's target speed is [R]

  v(u) = ((1 − u)·131·v₁₃₀ + u·7·v₁₃₁) / ((1 − u)·131 + u·7).

  | u | 0 | 0.25 | 0.5 | 0.75 | 0.9 | 1 |
  |---|---:|---:|---:|---:|---:|---:|
  | v(u), units/s | 38.3 | 48.8 | **68.7** | 121.2 | 233.2 | 638.4 |

  The other readings give other curves: the lift-to-plant intervals (49 and 25 frames) give 241 at u = 0.5 and
  whole cycles (180 and 32 frames) give 128.9. Those are not what sets the speed; the spec's ~69 stands. The body
  moves at this target speed (section 5.1 of physics.md: it is the velocity the ground contact imparts), while the
  blended animation plays at its own rate, so the feet may slide at mid deflection; that is the game's behaviour,
  not a port error. The curve is strongly non-linear; a check against the running game is still welcome.
- The same blend and marker rule, with the same two segment pictures (OIDs 190 and 191 carry identical root motion
  and markers), drives running along a rail. [D] [R]
- Sneaking (while flattened or hidden in shadow) caps the deflection at 0.5. [X] 0x274E58
- Turning: the speed is multiplied by clamp(1 − |Δθ|, 0, 1), Δθ (radians) between the facing and the stick's
  heading (physics.md 5.1). [X 0x274B30] [R]
- **Turn rate** [X] [R]: the facing turns toward the target heading by four-value smoothing (physics.md 4.3) of the
  heading error, giving a target turning speed; the angular acceleration needed to reach it within the frame is
  applied. Standing (and in every state other than running): (40 rad/s, 0, 0.25 s, 200 rad/s²) [X 0x263740].
  Running: the four values are interpolated toward (15, 0, 0.25, 100) [X 0x263750] by
  w = clamp(−0.2 + 0.002·speed, 0, 1) [X (−0.2, 0.002, 0) at 0x263760] (all "running" values from 600 units/s), and
  the fast and slow speeds are multiplied by clamp(0.2 + 0.8·u², 0, 1) [X (0.2, 0, 0.8) at 0x263770], u the stick
  deflection. Separately the body is kept upright (its up axis eased to vertical) with (10, 0, 0.5, 50)
  [X 0x263730]. Example: standing, a 90° turn asked from rest: the smoothing asks √(2·160·π/2) = 22.4 rad/s, the
  200 rad/s² limit allows 3.33 rad/s after the first 1/60 s; the turn takes about 0.18 s.

## 3. Jumping

| Jump | Initial upward speed | Tag |
|---|---|---|
| from the ground (standing or running) | enough for the origin to rise to **245 units** above the ground contact point: v = √(2 × 2275 × (245 − h)), h the height of the character's origin above that point at take-off. Standing, h = 90 (section 1.1): **839.8 units/s**, a rise of 155 units | [X] 0x263E84 (height) [R] [D for h] |
| from a swing, a pipe or a hook | 700·√1.3 = 798.1 units/s | [X] 0x263E80 |
| out of water | 900·√1.3 = 1026.2 units/s | [X] 0x263E88 |
| from a ledge | 800·√1.3 = 912.2 units/s | [X] 0x263E8C |
| toward a perch point or target | max(v_z, min(682 + 0.5·v_z, 682)) | [X] coefficients (682, 0.5, 0) at 0x264050, limits (−∞, 682) at 0x264060 |
| **boosted (second press)** | max(v_z, min(682 + v_z, 682)): rising slower than 682, the speed becomes 682; falling, 682 is added to it | [X] coefficients (682, 1, 0) at 0x264070, limits (−∞, 682) at 0x264080 |

In every case the jump never reduces an upward speed the player already has. Horizontal speed carries over and is
then steered in the air. [R]

**Second press of the jump button** = the boosted jump. [R] It is accepted:

- while in the air in the ordinary jump states (rising or falling; not during a smash, a dive or an aimed
  jump onto a target);
- **once per jump** (a second boost needs a new jump from the ground, a ledge, a pipe or similar);
- only if the ground is not about to be reached: a test point at the base point (physics.md 5.1), moved by
  v·0.1 + ½·g·0.1², must not hit anything (so a press in the last ~0.1 s before landing does nothing);
- at any time after take-off: no minimum delay. The boost plays segment 145.

**The 0.07 s and 0.30 s timers are not about the jump button.** [X 0x263E70, 0x263E74] [R] They watch the **stick**
right after a jump starts and decide how air steering behaves (the "coast" mode below):

- if the stick does not reach full deflection within 0.07 s of the jump, the jump is **short-steered**;
- if it reaches full deflection but leaves it again less than 0.30 s later, the jump is short-steered too;
- releasing the jump button before the end of take-off (0.15 s, [X 0x263E50]) also makes it short-steered;
- jumps from rails, ledges with no stick and some others are short-steered from the start.

The earlier wording ("the second press is recognised from 0.07 s and fully from 0.30 s") was a misreading.

**Air steering.** [R] [X for the rates] Let s be the current horizontal speed, along direction ĥ. The stick gives a
wish vector w of length u × 518.8 units/s (section 1; × 0.7 after a jump out of water, × 0.6 after a ledge jump
with no stick). The coast target c is:

- w itself during take-off and in a short-steered jump;
- otherwise (momentum is kept): if w's part along ĥ is at least s, then w; if it is above −0.25·s, then s·ĥ plus w's
  sideways part; else w.

Expressed along ĥ (forward) and across it (side), the horizontal acceleration per second is
rate_f × (c_forward − s) forward and rate_side × c_side sideways, with

| Phase | rate_f when speeding up (s ≤ c_forward) | rate_f when slowing | rate_side |
|---|---:|---:|---:|
| take-off (first 0.15 s) | 60 | 20 | 10 |
| rest of the jump | 2.5 | 3.5 | 2.5 |

The six rates are stored at 0x263E58 to 0x263E6C (2.5, 3.5, 2.5, 60, 20, 10) [X; the assignment of each word to a
row is from the reference]. Vertical: gravity plus the terminal-fall term (physics.md 2).

- While the horizontal speed exceeds 300 units/s the facing target follows the direction of motion [X 0x263E54].
  A target jump's path is searched up to 10000 units [X 0x264048].
- A smash or dive needs at least 0.35 s in the air [X 0x264040, 0x264044].
- Ledge grab check: within 60 units horizontally and 70 units vertically of an edge. [R]

## 4. Climbing, rails, hanging

- **Ledges**: a grabbed ledge holds the player still; there is **no shimmy** in Sly 1. The player is pulled to a
  hang position 37 units out from the edge (away from the wall) and 25 units below it [X (37, 0, 25) at 0x2636F0]
  [R], easing in with smoothing (1000, 0, 0.25); the only way off is the jump button, which does the ledge jump
  (section 3). Segment 139 is the hang. [R] The offset is measured from the player's **origin**, not his feet: the
  origin sits 25 units below the edge, which puts the feet about 115 units below it (section 1.1).
- **Pipes**: the climbing speed is not stored; it comes from the animation. Each of the two climb segments (OIDs
  155, slow, and 156, fast) has no root motion [D]; the game takes the vertical velocity of the **right foot**
  relative to the character at the segment's start (time 0) as that segment's climb speed (the foot pushes down,
  the body climbs). With stick deflection u: speed = (1 − u)·s₁₅₅ + u·s₁₅₆, and the two segments are re-blended
  so the animation matches the speed. Near the pipe's ends the speed is limited by a smoothing (|s₁₅₆|, 0, 0.2)
  toward the limits: hands stop 35 units from a pipe's end, feet 100 units [X 0x274F00, 0x274F04]; reversing
  needs a stick change of 0.5 [X 0x274F08]; pipes are probed 50 units around [X 0x274F14]. [R]
  Measured on the disc [D]: the right foot (OID 58, the foot's "ball" object, a direct child of the player) moves
  at **−66.0 units/s** along the player's z at the start of segment 155 and **−365.3 units/s** at the start of 156
  (same in every world checked), so the climb speeds are **66.0 and 365.3 units/s**: speed = 66.0 + 299.3·u.
- **Rails**: running along a rail uses the run blend of section 2 (OIDs 190 to 195: slow, fast and their up/down
  slope variants), so the speeds are those of section 2; a rail may also set its own maximum speed. Reversing
  direction needs a stick direction below −0.2 against the rail [X 0x263990]; the balance segments 283 to 285
  (stationary) are blended by the balance state. [R]
- **Hooks (swinging)**: the swing segments 343, 344, 345 (40 frames each; forward, neutral, backward) are blended
  by the pendulum's phase; the swing jump is in section 3. [D] [R]
- **Perch points and jump guides**: a jump near a perch point is aimed at it (section 3) with drive mode 4 of
  physics.md 4.4 (a timed curve that lands exactly on the point). [O] [R]

## 5. Wall and hiding moves

| Move | Segments | Measured speed (Paris) [D] |
|---|---|---|
| flatten against a wall, sidestep | 159 to 164 (80 frames each) | 120 units/s (two of them) and 135 units/s (four) |
| flatten, stand and peek | 165 to 169 | stationary |
| duck walk | 170 to 177 | stationary root (moved by the blend) |
| barrel walk | 181 (38 frames), 182 (18 frames) | 72.6 and 416.7 units/s |

Entering or leaving a flattened pose smooths over 0.3 s [X 0x274E48]; the minimum sidestep speed while flattened is
25 units/s [X 0x274E4C].

## 6. Attacks and buttons

Buttons (PS2 names) [R] [O]: Cross jumps, Square attacks, Circle is the context button (hide, grab, flatten),
**Triangle uses the selected power-up**.

| Attack | Input | Segment | Motion |
|---|---|---|---|
| sweep forward/back, moving | Square on the ground | 243, 245, 247, 249 (40 frames) | in place [D] |
| sweep, standing | Square, standing | 251 to 257 (40 frames), returns 259, 260 | in place [D] |
| **rush** | Triangle on the ground, when the selected power-up is the dive power-up and reward bit 0x0002 is held | 261 (60 frames) | 550 units forward in 1 s (550 units/s) [D] |
| **smash** | Square in the air, after 0.35 s | 263 (40 frames) | no speed of its own: the player keeps falling with gravity and the terminal term; the move only turns him toward the nearest target [R] |
| **dive** | Triangle in the air with the dive power-up selected and reward bit 0x0010, after 0.35 s | 440 (120 frames) | see below |

So the rush has no button of its own: it is the ground form of the dive power-up, and the dive is its air form
(together the reward mask 0x0012 of section 8.1). [R]

**Dive** [D] [R]: labels in segment 440: 442 and 312/316 at frame 20, 443 and 317 at frame 100. Gravity is cancelled
from the start until label 443 (frame 100); the horizontal velocity, and the vertical velocity until label 442, are
pulled toward zero at a rate of 10 per second; between frames 20 and 100 the segment's root motion becomes the
character's velocity (labels 312/317, animation.md 7): the root drops 2075 units in 80 frames, **1556.3 units/s
straight down**. After frame 100 ordinary gravity resumes. So the dive is a 0.33 s hover followed by a 1556 units/s
plunge.

**Reach** [D] [R]. Hits are made by small solid "effect" objects under the player, enabled during the attack:
their contacts with targets do the damage (physics.md 3.2). Their shapes, in their own space (each is a child of
the player or of one of his bones, placed at the position shown):

| Attack | Effect object (OID) | Placed at | Shape extent (x; y; z) |
|---|---|---|---|
| sweep | 126 | (0, 0, −55.4) | −83.9 to 28.8; −201.2 to −27.6; −60 to 40 |
| smash | 127 | (4.8, 0, 29.8) | ±50; ±50; −1.4 to 148.6 |
| rush | 128 | (75.8, 0, 18.6) | ±25; ±87.5; ±87.5 |
| dive | 441 | (−6.9, 0, −40.0) | ±54.9; ±53.6; ±42.1 |

(The sweep's shape follows the cane bone through the swing.) **Aiming**: when an attack starts, the player turns
toward the best attack target (target markers, default radius 25): the one closest to the attack's natural hit
point, (175, 0, 0) for the sweep, (250, 0, 125) for the rush, (125, 0, 0) for the smash in the player's frame,
with both points predicted 0.15 s (sweep) or 0.25 s ahead, within 300 units (550 for a character, which gets a
250-unit bonus), and within ±1.6 rad of the heading for a moving sweep, ±1 rad for the rush, any direction for a
standing sweep or a smash. [R]

- After a smash, the contact window lasts 0.25 s. [X 0x26399C]
- A dive landing affects objects within 300 units and knocks coins loose within 500 units. [R]

## 7. Damage and death

Hazards and attacks pass a damage kind (interactables.md 5, alarms-sensors.md). The reaction and the segment
played, by kind and by whether a charm is left [R], with the segments' root motion [D]:

| Kind | With a charm | Without a charm |
|---|---|---|
| 0 blunt (hits) | 233, then 226 while the charm is used | 233, then death |
| 1 electric | 227, then the charm is used | 227, then death |
| 2 fire | 228, then the charm is used | 228, then death |
| 3 water | **229**, the charm is used | **235** (sinks: the root drops 337 units over 302 frames) |
| 4 crush | 230, the charm is used | **238** |
| 5 pit | **231**, the charm is used | **237** (falls: the root drops 1920 units over 229 frames) |
| 6 thrown | 233, then 232 (169 frames), then the charm is used | 233, 232, then death |

"Death" (after kinds 0, 1, 2, 6) is segment **236**, or **234** when the player is moving backward faster than 250
units/s relative to his facing; in that case he is turned toward facing away from his motion by a fraction
clamp(−1 + 0.004·speed, 0, 1) of the angle (0 at 250 units/s, all of it at 500) [X (−1, 0.004, 0) at 0x2639A0].
(The earlier description of this rule as "death in water" was wrong.) A death that happens while jumping at a
pipe, rail, perch, hook or hand-hold, with a charm left, uses the charm and lands on the target instead. Every
hit with a charm left uses one charm and flashes the player for 2 s.

**Crush velocity**: on any crush hit (with or without a charm) the player's velocity is set to **(0, 0, −1000)**
units/s, straight down (world axes), replacing his velocity [X 0x275040] [R]. There is no sideways knock-back.

**Repeated hits**: there is no repeat timer. Damage is ignored while the player is already in a damage reaction,
in a scripted (puppet) state, while flashing after a charm hit (2 s), or with the hands-off flag set (blunt,
electric, fire, crush, thrown). Water and pits ignore flashing; they are ignored instead while the player is on a
hook, a pipe or a ledge, in an aimed jump, already in a water or pit reaction or dying, or hidden (except the
casual and reach hiding kinds). [R]

## 8. Power-ups

| Power-up | Effect | Tag |
|---|---|---|
| slow motion | game time × 0.25 while active (clock-and-loop.md 3) | [R] |
| speed-up | game time × (1 + 0.5 u) | [R] |
| decoy, mine | spawn their objects (classes 76, 63); segments 339 to 342 | [D] [O] |
| rolling ball | see below | [R] [X] |

**Rolling ball** [R]: Triangle held turns the player into a ball of radius **50 units** [X 0x262E30] (segment 282).
Both other values are speeds in units per second:

- **850 units/s** [X 0x262E2C] is the launch speed: when rolling starts, if the player already moves at 150 units/s
  or more, his velocity keeps its direction and is raised to at least 850 units/s; below **150 units/s**
  [X 0x2639B0] he is launched at 850 units/s along his facing.
- While rolling, the stick asks for a spin of (u × 1000) / 50 = 20·u rad/s about the horizontal axis across the
  stick direction, reached with a damping rate of 2.5·u per second; the ball's contacts have friction 0.8 and
  elasticity 0.5. The camera uses the direction of motion, since the model spins.
- Releasing Triangle after at least 0.25 s ends the ball and the player recovers upright.

## 8.1 Rules defined in other specs

- **Hidden from guards**: when flattened, ducking in cover, in a hiding place, or (with the power-up mask
  `0x12000`) in a fourth hiding mode, the player counts as hidden by the rules of guards.md 4.3; sneaking movement
  limits are in section 5 above.
- **Damage kinds**: section 7. **Water and pits do not bypass charms**: with a charm left, falling into water or a
  pit uses the charm (reaction 229 or 231) and the player is put back; only without a charm do they cost a life.
  [R] This replaces the earlier [I] guess.
- **Power-ups from vaults** (pickups-and-progress.md 4.2): each power-up needs bits of the reward mask [R]:
  dive attack 0x0012 (0x0002 rush on the ground, 0x0010 dive in the air), rolling ball 0x0004, slow motion 0x1008,
  mine 0x0040, speed-up 0x0080, decoy 0x0200, stun 0x8000; bit 0x0020 pulls coins and clue bottles toward the
  player. The power-up in use is chosen with a button and saved as the last power-up selected (save-format.md).

## 8.2 Animation OIDs by state

Entries of the player's lookup list (animation.md 7) [D for the OIDs and segment lengths, R for the roles]:

| State | OIDs |
|---|---|
| idle | 325 is not a segment but the idle **state machine** object (it chooses the idle variations); standing still between idles is the run blend at u = 0 [I] |
| run (ground) | 130 slow, 131 fast; 132 to 135 the uphill and downhill variants (section 2) |
| jump | 140 standing jump, 141 running jump, 142 jump from a swing, a pipe, water or a ledge, 144 aimed (target) jump, 145 boosted jump |
| fall | the current jump segment from its label 308 (frame 25); a fall that does not start with a jump (walking off an edge) starts the standing (140) or other jump segment at label 308 |
| ledge hang | 139 (380 frames) |
| hook swing | 343, 344, 345 |
| perch | landing on a perch point uses the aimed jump (144) and its landing; a separate perch-pose OID is not identified (open question 6) |
| pipe | 158 enter, 154 stay, 155/156 climb, 157 turn round; 221 and 222 enter and leave from the ground |
| rail | 190 to 195 run blend, 283 to 285 balance |
| flatten, duck, barrel | 159 to 166 flatten sidestep blend, 167 flatten stand, 168/169 flatten peek right/left, 170 to 177 duck blend, 178 duck stand, 179/180 duck peek right/left, 181 to 186 barrel run blend, 187 barrel stand, 188/189 barrel jump in and out; 146 to 153 another eight-way sidestep blend (use not identified) |
| attacks | 243 to 260 sweeps, 261 rush, 262 rush bounce, 263 smash, 264 smash bounce, 440 dive |
| damage | 226 to 238 (section 7) |
| ball, power-ups | 282 ball, 339 decoy, 340 stun, 341/342 mine place and activate |

## 9. Test vectors

| Case | Expected |
|---|---|
| standing jump on flat ground (origin 90 above the contact point) | leaves at 839.8 units/s; apex 155 units above the take-off height after 0.3691 s; the fall back (155 units) stays above the terminal threshold (reached only after 160.7 units), so it **lands after 0.7383 s** at −839.8 units/s |
| the same rule with the origin at the contact point (h = 0; for checking the terminal term) | leaves at 1055.8 units/s; apex 245 after 0.4641 s; lands after 0.9293 s at −1019.1 units/s (0.9291 s at 60 frames/s); without the terminal term 0.9282 s |
| jump from a swing (v_z before = 0) | apex 140.0 units after 0.3508 s |
| boost while falling at −500 units/s | new v_z = 182 units/s |
| boost while rising at 300 units/s | new v_z = 682 units/s |
| full-deflection run on flat ground, steady state | 638.4 units/s |
| half deflection | 68.7 units/s |
| no deflection, walk cycle | 38.3 units/s |
| rush attack from standstill | 550 units forward after 1.0 s |
| dive | 0.333 s hover, then 1556.3 units/s down for 1.333 s |
| rolling ball from standing | 850 units/s along the facing |

## 10. Open questions

1. Ground acceleration and deceleration while running (the turn rate is now in section 2).
2. The mid-deflection run speed of section 2 should still be checked by measurement in the running game.
3. (Settled: section 4 gives 66.0 and 365.3 units/s.)
4. Input dead zones and the stick response curve. (Searched: no dead-zone or turn-rate value is stored as a data word
   near the player's other constants; they appear to be immediate values in code. Menu navigation thresholds are
   in ui-and-menus.md 7.)
5. The [R] constants above (gravity −1750 and ratio 1.3, ledge-grab box, dive radii) are not stored as data words
   in the executable; they are probably immediate operands in code and are confirmed only by the reference.
6. A dedicated perch pose, if any.
