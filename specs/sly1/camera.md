---
id: sly1-camera
title: Sly 1 camera
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46
      worlds; executable SCUS_971.98 sha256 d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c
    used_for: >-
      camera-zone records and options (442 zones), camera placements (101), world camera options; constants read
      from the executable at the addresses given
  - kind: observation
    what: the game as played
    used_for: right-stick orbit, recentre button, automatic follow, look-around mode, cutscene cameras
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files cm, cptn, cplook, cpman, cpalign, cpaseg, tn, util, spaprops, po, so, jt
    used_for: >-
      camera modes, zone framing rules and spaces, the focus point, springs and smoothing, orbit limits, view
      clearing, zone option meanings; [R]
date: 2026-11-17
---

# Camera

Tags: [D] disc data, [X] constant read from the executable at the given virtual address (file offset = address −
0x100000 + 0x1000; the port checks the executable's SHA-256 in disc.md first and refuses on a mismatch), [O]
observable, [R] reference behaviour, [I] inferred. Units: game units, seconds, radians.

## 1. Lens and fog

| Setting | Default | Per-world override (world options) |
|---|---|---|
| field of view | 1.0 rad [R]; **vertical**, full angle: the vertical half-extent of the view is tan(fov / 2) and the horizontal one tan(fov / 2) × 1.2987 [R] | option 529 (Paris: 1.1) [D] |
| aspect ratio | 1.2987 (about 4:3 including the PS2 pixel shape) [R]; the port uses the display's aspect | — |
| near / far clip | 100 / 20000 [R] | options 530, 531 (Paris: 70 / 60000) [D] |
| fog start / end / maximum | 500 / 20000 / 0.5 [R] | options 532, 533, 534 (Paris: 1000 / 30000 / 0.4) [D] |
| fog colour | — | option 535 (packed colour) [D] |

Changes of field of view are smoothed with smoothing parameters (0.5, 0, 0.1, 10) [X 0x2618B8] (fast speed 0.5
rad/s, slow 0, fast time 0.1 s, and a fourth value 10: the change of the field of view accelerates by at most
10 rad/s²; physics.md 4.3 gives the smoothing law). A zone can set its own field of view (option 878, section 3.5).

## 2. Modes

At any time the camera follows one mode: [R] [O]

| Mode | When | Behaviour |
|---|---|---|
| zone | normal play (almost always) | framed by the camera zone the player is in, or the default framing (section 3) |
| look | the look-around view (the binocucom) | the eye sits near the player's head and turns with the stick (section 6) [I for the binocucom link] |
| animation | cutscenes and scripted shots | the camera follows a camera placement (class 78) driven by an animation segment |
| aligned | some scripted moments | the camera keeps a fixed alignment to an object |
| manual | development | free camera |

A **cut** (a mode switch, a cutscene start, a warp) snaps the camera to its new place in one frame; otherwise every
change is smoothed (section 4).

## 3. Camera zones (class 99)

### 3.1 Data and spaces

- A zone record holds [D] [R]: the zone object's own transform (rotation and position), its convex volume
  (classes/markers-and-volumes.md 7), a path kind, then either (path kind −1) a **fixed frame** (a second rotation
  and position) or (any other kind) a **path curve**, then its options.
- **Spaces**: the fixed frame and the path are both in the **zone's own space**, the space of the zone object's
  transform (and of its parents, if any). The focus is converted from world space into the zone's space, framed
  there, and the resulting eye position is converted back to world space. [R] So the [I] of the implementers'
  reading ("the zone's transform is applied to its path and fixed frame") is confirmed.
- The player is in a zone when inside its volume. When several zones contain the player, the one with the highest
  priority (option 870) wins. [R] Option 871 lets a zone take over while the player is in the air. [R]
- Allowed framing directions (option 867, bits: 1 forward, 2 reverse, 4 manual): default 5 (forward and manual) for
  a fixed frame and 7 for a path. [R] On the disc 867 takes the values 1, 4 and 5 [D].

### 3.2 The zone frame of the focus

- **Fixed frame**: the focus offset is the focus position minus the frame's position, expressed in the frame's
  axes (x forward, y left, z up).
- **Path**: the frame sits at the point of the path closest to the focus (searched from the previous closest point,
  or over the whole path when the zone is entered), with x along the path's tangent at that point, z up and y
  completing the frame; the focus offset is expressed in that frame.

### 3.3 Framing

In the zone frame, each component c of the focus offset is mapped by its own quadratic and clamped:
oᵢ = clamp(a₀ + a₁·cᵢ + a₂·cᵢ², minᵢ, maxᵢ) (options 852, 854, 856 for x, y, z; 853, 855, 857 for the limits,
which never occur on the disc). [R] [D] Then:

- **Fixed frame, forward**: the eye is at frame position + frame axes × (o_x + d, o_y, o_z), d the distance of
  option 851 (negative = behind).
- **Path, forward**: the eye is at the path point d further along the path from the closest point (d negative =
  back along it), plus that point's frame axes × (o_x, o_y, o_z).
- **Reverse framing**: o_x is negated, and on a path the eye is d back along the path instead of forward. The
  preferred screen x (859) is negated too.

**Cylindrical zones** (option 866 = 1; 36 zones on the disc [D]): the focus offset (in the fixed frame or the path
frame) is first turned into cylindrical form around the frame's z axis: (angle θ, horizontal radius ρ, height h).
[R]

- **Slack** (option 858; 0, 0.15 or 0.25 on the disc): except on the frame where the zone is entered, θ follows
  the focus only beyond ±slack from its previous value (the previous value moves just enough to keep the focus's
  angle within the slack).
- The three quadratics of 852, 854, 856 then apply to (θ, ρ, h), not to (x, y, z), and their results are turned
  back from cylindrical to Cartesian in the frame: (o_θ, o_ρ, o_h) → (o_ρ cos o_θ, o_ρ sin o_θ, o_h).
- **The framing distance** (851) is applied after that: on a fixed frame it is added to the x (forward) component
  of the Cartesian result, along the frame's own x axis, not along the radius; on a path it moves the frame point
  along the path as above. In the cylindrical zones on the disc 852 is typically the identity on the angle and 854
  sets the radius, so the camera sits on a circle around the frame's z axis at the angle of the focus.

### 3.4 Default framing (no zone, and the starting values of every zone) [X 0x275980]

| Field | Value | Offset in the block |
|---|---|---|
| distance (851) | −450 | +0 |
| x, y offset quadratics (852, 854) | identity (a₀ = 0, a₁ = 1, a₂ = 0) | +16, +32 (16-byte records) |
| z offset quadratic (856) | 125 + z | +48 |
| limits (853, 855, 857) | unlimited | +64 (three (min, max) pairs) |
| preferred screen position of the focus (859, 860) | x 0, y −0.1 | +88 |
| adjust, follow and lock-on factors (861, 862, 863) | 1, 1, 1 | +96 |
| focus scale (865) | (1, 1, 1) | +112 |
| eye spring strength and damping factors (885, 886) | 1, 1 | +128, +132 |

With no zone the camera is in manual framing: its horizontal distance from the focus is |851| = 450 and its height
above it is 856's constant, 125; the angle comes from the orbit (section 5).

### 3.5 Zone options

Meanings of every zone option, with use on the disc [D] and behaviour [R]:

| Option | Uses | Meaning |
|---|---:|---|
| 851 | 362 | framing distance d (3.3) |
| 852, 854, 856 | 205, 299, 372 | x, y, z offset quadratics (3.3) |
| 853, 855, 857 | 0 | their limits |
| 858 | 14 | cylindrical slack angle (rad) |
| 859, 860 | 87, 273 | preferred screen position of the focus |
| 861 | 132 | adjust factor (section 4.1): how far the framed point moves from the player's base toward the look-ahead point |
| 862 | 47 | automatic-follow factor (section 4.3) |
| 863 | 90 | lock-on factor (section 4.3) |
| 864, 865 | 54, 88 | focus scale: 865 a vector; 864 a ratio u setting it to (u, u, 1) (section 4.1) |
| 866 | 36 | cylindrical zone |
| 867 | 193 | allowed framing directions |
| 868, 869 | 191, 63 | pan on entry, pan when idle (on/off) |
| 870, 871 | 215, 38 | priority; take over in the air |
| 872 | 0 | how sideways motion is measured for the automatic orbit: behind (default) or side |
| **873** | 5 (value 0) | **pan-on-entry time** (s, default 2): after entering, the camera turns toward the framed direction with a strength that ramps from 0 to 1 over this time; 0 means at once |
| **874** | 72 (2 to 4) | **pan-on-entry strength** (default 1): the ramp's final strength |
| **875** | 0 | **pan-when-idle time** (s, default 2) |
| **876** | 9 (1 to 3) | **pan-when-idle strength** (default 1). The idle pan starts 0.75 s after the player last moved and at least 10 s after the last manual orbit, and ramps in over the idle time |
| **877** | 13 (1) | **cut on entry**: entering the zone cuts the camera instead of easing |
| **878** | 13 (0.5 to 1.1) | **field of view** while in the zone (rad, vertical); the previous one is restored on leaving; changes are smoothed (section 1) |
| **879** | 18 (1) | **no view clearing** in this zone (section 7 is skipped) |
| **880** | 0 | a **follow target** object (set by scripts): the automatic follow then measures the motion toward that object instead of the player's velocity |
| **881** | 2 ((900, 1, 0), (500, 1, 0)) | speed of that follow motion as a quadratic of the distance to the target |
| **882** | 2 ((0, 1500)) | limits of that speed |
| **883** | 0 | the zone's state (read by scripts: in, out) |
| **884** | 1 (0) | **state override** [R]: −1 no override (the default when the option is absent), **0 forced out**, 1 forced in, regardless of the volume. The disc's one use (value 0) therefore switches that zone off until the game or a script changes it. The game also sets it at run time for hiding places and jump guides. Stored as a 4-byte integer like other enumerated options |
| **885** | 1 (0.5) | **eye spring strength factor**: multiplies the 15 of section 4.2 |
| **886** | 1 (1.0) | **eye spring damping factor**: multiplies the −8 of section 4.2 |

## 4. Focus, following and smoothing

### 4.1 The focus point

Built each frame from the player [R] [X where marked]:

1. **Base point B.** Horizontally the player's origin (x, y). Height: a ground height G that is the height of the
   player's ground contact point (physics.md 5.1) while standing on something; on a hook, the hook's height − 300;
   kept unchanged while in the air; on a camera cut, the origin's height − 75. G is clamped to between 350 and 50
   units below the origin. B_z eases toward G + 125 with smoothing (1000, 10, 1.0) [X 0x269BC0] and is clamped to
   between 325 below and 75 above the origin (on a cut it is set to G + 125 at once). On a pipe B is the hand
   point on the pipe.

   **Which origin.** All the clamps are measured from the player's **origin**, which stands 90 units above the
   ground contact point (player-moves.md 1.1), not from his feet. Standing: G = origin − 90 (inside the
   [origin − 350, origin − 50] clamp), B_z = G + 125 = origin + 35 (inside [origin − 325, origin + 75]). So B is 125
   above the ground and 35 above the origin; neither clamp acts. A port that measures from 50 above the feet puts
   its origin 40 units too low. During a jump G is kept until the origin rises more than 260 units above its
   take-off height (G then follows at origin − 350) or drops more than 40 units below it (G follows at
   origin − 50, which is what lets the camera go down with a fall).
2. **Look-ahead point A** = B + an adjustment vector. The adjustment eases toward a point 100 units ahead of the
   player along his facing (0 in some hiding places) with a spring of strength 5 and damping −3.5 (velocity first,
   then position).
3. **Nose N** = B + (A − B) × the zone's adjust factor (861; default 1, so by default the framed point is the
   look-ahead point).
4. In a zone, N is expressed in the zone frame (3.2) and each component scaled by the focus scale (865, or
   (u, u, 1) from 864), then converted back: a scale below 1 pulls the framed point toward the frame's origin or
   the path.
5. **Focus F** = N + a focus offset. The focus offset follows the player's moves:
   - when a move sets a focus point P of its own (flattened against a wall: 50 units above the origin, shifted
     sideways by up to 125 units toward the side the player looks; and similar moves), the offset springs toward
     P − N with strength 8 and damping −6;
   - when the player is relaxed, it springs back to zero with strength 5, damping −3.5;
   - in the clamped state it only decays (damping −10).

   All these springs are integrated velocity first, then position; on a cut they snap to their targets.

There is no fixed "focus height above the feet": it is the ground height + 125 (eased), plus the look-ahead.

### 4.2 The eye spring and the eye

The camera keeps the eye in cylindrical form around the focus: angle θ, horizontal distance ρ and height z. Each
frame [R]:

1. The framing (section 3) gives target values (θ_t, ρ_t, z_t); with no zone, ρ_t = 450 and z_t = 125.
2. θ comes from the orbit and lock rules (4.3, section 5); it is not sprung.
3. ρ and z each follow their targets with the spring (15·s₈₈₅, −8·s₈₈₆) [X 0x261920 for 15 and −8]:
   v ← (1 − 8·s₈₈₆·dt)·v + 15·s₈₈₅·(target − x)·dt, then x ← x + v·dt (velocity first, then position).
   On a cut the velocities are zeroed and the targets taken directly.
4. View clearing (section 7) adjusts the eye.
5. The eye is then pushed **outward** along the focus–eye line by the near-clip distance:
   eye = F + e × (|e| + near) / |e|, e the eye offset. So the visible distance is the framed distance plus the near
   clip.
6. The view is aimed so the focus appears at the preferred screen position (859, 860; default (0, −0.1)).

**Settling of the spring.** The spring (15, −8) is over-damped (ζ = 8 / (2√15) ≈ 1.03): its characteristic rates
are 3 and 5 per second. From rest, a step of size D is covered as D·(1 − (5e^(−3t) − 3e^(−5t))/2): **88.6 % after
1 s**, 99 % after 1.84 s, no overshoot. The earlier "settles within about 1 s" was loose; the dominant time constant
is 1/3 s. At 60 frames/s with the integration of step 3: 88.6 % after 60 frames, within 1 % from frame 114
(1.9 s), at most 99.96 % (no overshoot).

### 4.3 Following

- The automatic-follow factor eases toward the zone's value (862), or toward **0** while the player is: in an
  ordinary jump that has not used its boost yet (rising or falling), swinging on a hook, in a damage reaction, in
  the sidestep state, or hiding in the casual, reach or vault kinds (not when flattened, ducking, in a basket, on
  a rail or a perch) [R]
  with smoothing (4, 0, 0.25) [X 0x275A80].
- **Automatic orbit** (manual framing only): θ turns at (sideways speed / camera distance) rad/s, the sideways
  speed being the player's velocity (or the follow-target motion, 880) measured across the camera's horizontal view
  direction (or along it, 872 "side"), capped at 2.5 rad/s, times the follow factor, times a factor
  clamp(1 − 2y − 2y², 1, 2) (y the left stick's forward deflection, so pulling the stick toward the camera turns
  it up to twice as fast) [X (1, −2, −2) at 0x275A40, limits (1, 2) at 0x275A50], times clamp(time since the last
  manual orbit, 0, 1) [X (0, 1, 0) at 0x275A70].
- **Turning to the framed direction** (zones with forward or reverse framing): θ eases toward the framed angle
  with smoothing (0.5·L, 0, 0.5) [X the record (0.5, 0, 0.5) at 0x275A58], L the lock strength:
  L = min(1, horizontal player speed / 600) (/200 when flattened) × the zone's lock-on factor (863); the pan on
  entry and the idle pan (873 to 876) raise L to their ramps when larger. How much of this easing is used depends
  on how well the player's facing matches the path: with c the cosine between the facing and the path direction,
  the weight is c²·(3 − |c|) (1 on a fixed frame, 3 in a hiding place's zone). Once the angle crosses the framed
  angle the framing switches from manual to forward (or reverse).

## 5. Player input

- **Right stick (horizontal)**: manual orbit. Target angular speed = x·|x|·2.5 rad/s (x is the stick's horizontal
  deflection, −1 to 1; inverted when the options say so); the angular speed changes by at most 15 rad/s² toward
  the target. [R] Any manual orbit switches the zone's framing to manual until the player moves on. [R] [O]
- **Right-stick press**: recentre behind the player with smoothing (2.5, 0, 0.35, 15) [X 0x275A90]; plays a click
  sound. [R] [O]
- The camera's input runs on the real clock unless the world-map camera is active. [R]

## 6. Look-around mode

The eye is placed 70 units above the player's anchor, offset toward the look direction between −25 (near) and −150
(far) along the look axis [X 0x261C10, 0x261C20 for the two offsets; 70 is [R]]; the view turns with the left stick.
[R] [O] Its use as the binocucom view is inferred.

## 7. Keeping the view clear

Two mechanisms, both skipped in zones with option 879. [R]

**Squish (every frame).** The step of the eye from its previous position to its new framed position is limited
so the eye stays clear of camera meshes (collision.md 4; scenery marked as camera-fade fades instead and is
ignored here):

- **Search box ±250** [X (−250, −250, −250) at 0x261970]: the solids considered are those whose bounds touch a box
  of ±250 units around the eye's **previous** position.
- **Eye sphere radius 250**: the camera meshes of those solids are intersected with a sphere of radius 250 around
  the previous eye position; every surface found gives a plane (normal n, distance g from the sphere's surface).
- **Clearance**: the eye wants to stay at distance r_near from each surface, where r_near is the radius of the
  near-clip rectangle's corner, r_near = tan(fov/2)·√(aspect² + 1)·near + 1 (90.5 with the defaults fov 1.0,
  aspect 1.2987, near 100; 71.3 in Paris, fov 1.1, near 70).
- For each plane, the eye's velocity along n must be at least the speed that the smoothing (2500, 0, 0.25)
  [X 0x261958] gives for moving from g to r_near − 250: when the eye is farther than the clearance this allows it to
  approach no faster than that law, and when it is closer it is pushed out at that speed.
- The smallest correction that satisfies all planes (a sum of the plane normals with non-negative weights) is
  added to the eye's velocity; its length is capped at the eye's own speed + 1500.
- **The record (250, 1500, 0) at 0x26194C** [X] is this check's own record: 250 the sphere radius used by the
  constraint and 1500 the extra speed allowance; its third value is unused. The **2500 that follows it belongs to
  the next record**, (2500, 0, 0.25) at 0x261958: the two were read together as one four-value record by mistake.

**Cut when blocked.** From 0.5 s after the zone became active (or after the last clear check) [R], each frame
five probe points around the player are tested: his origin, ±50 units sideways, 75 up and 50 down (sideways
relative to the camera's view). If the line from any of them to the matching point on the near-clip plane is clear,
the timer restarts. If none has been clear for **2 s**, the camera cuts. On a cut the eye is placed at a clear
position, in three steps [R] [X for the sample table]:

1. **Nearest clear point C.** From the player's origin, rays of length 500 are cast to sample points on a sphere:
   tilts of −90° to +90° in steps of 22.5°, with 1, 5, 10, 12, 15, 12, 10, 5, 1 pan samples at those tilts
   [X 0x261928], pan angles i·180°/n for i = 0 … n − 1 (only half a turn, as the executable does; the pan is the
   world angle from +x toward +y). Along each ray, the first stretch clear of camera meshes that is longer than
   10 units (2 × 5) is found; its start u (0 if the origin itself is clear, otherwise plus 5 units) is noted. C is
   the point at that start on the ray with the smallest u; if no ray has a clear stretch, C is the origin.
2. **Eye.** The framed eye, if the line from C to it is clear (else the end of its clear stretch from C);
   if the line is blocked before reaching it, the best of a search of angles around the focus (steps of about 16°, up to ±180°, preferring small
   turns and long clear lines).
3. The result is squished as above with r_near as the sphere radius.

## 7.1 Dialogue panels and letterbox

- **Picture panels** (dialogue-and-voice.md 3): radio and briefing dialogues show speakers in left and right
  panels; each panel is drawn by its own small camera (field of view option 989, default 0.5 rad; distance option
  990, default 400; light offset 993). The main camera keeps running behind the panels. [R]
- **Panel size and shape** [R]: each panel is **160 × 125.4** units of the 640-wide frame (scale with the display
  like the HUD), placed by the HUD placement table, elements 26 (left) and 27 (right): bottom left and bottom right,
  20 above the bottom edge (hud.md 2). Its outer side (toward the screen edge) is straight and its inner side is a
  half circle of radius 62.7 (= half the height): a straight part 97.3 wide and a round end. A 3.3-unit outline
  surrounds it. Over the picture lie 57 horizontal bands alternating black and mid grey, each at alpha 48/128 (a
  television look), each band shifting sideways a little with its own random smoothing.
- **Options 991 and 992 are not the panel's position**: they place the **speaker inside the panel**, as fractions of
  the panel's width and height (0 = left or top edge, 1 = right or bottom edge); the panel camera looks at the
  speaker so that he appears at that point. [R]
- **Confrontation dialogues** switch the main camera to a conversation view anchored at the dialogue's anchor point
  (option 967 or a line's anchor) and lock the player's controls. [R]
- **Letterbox** (dialogue events 5 and 6, animation events 49 and 50) draws black bars over the top and bottom of the
  picture; it does not change the camera's field of view. [R] [O]

## 8. Test vectors

| Case | Expected |
|---|---|
| no zone, player standing still facing +x on flat ground, everything settled | focus 100 units ahead of the player (+x) and 125 above the ground; framed eye offset 450 back and 125 up; after the near-clip push the eye is at focus + (−546.4, 0, 151.8) with near 100 (focus + (−517.5, 0, 143.7) in Paris, near 70); the focus drawn 0.1 of the screen below centre |
| right stick fully right from rest | angular speed after 1/60 s: min(2.5, 15/60) = 0.25 rad/s; reaches 2.5 rad/s after 0.1667 s |
| eye spring, 100-unit step of the framed distance from rest | 88.6 units after 1 s, 99 after 1.84 s, never more than 100 |
| zone with 885 = 0.5, 886 = 1 | spring (7.5, −8): ζ ≈ 1.46, slower (rates 1.1 and 6.9 per second) |
| clearance radius | 90.5 units (fov 1.0, near 100) |

## 9. Open questions

1. Zone hand-over between overlapping zones in detail (the framing switches to manual on entry and eases to the new
   framed angle; the order of evaluation with 871 is from the reference only).
2. The look-around mode's input mapping and limits; whether it is the binocucom view.
3. The squish solver's exact treatment of several planes (a small non-negative least-squares problem; the port may
   use any method that gives the minimal correction).
4. Cutscene camera placements: how placement objects define the lens (field of view animation).
