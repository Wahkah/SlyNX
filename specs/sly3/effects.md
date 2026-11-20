---
id: sly3-behaviour-effects
title: Sly 3 effects - emitters, effect objects and their components, sky - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), every class-27, 30, 31, 45, 110 body and every effect component (classes 191, 192) of all
      39 worlds, untruncated option survey; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: option counts and values; value types and fields from the option table at 0x32CA80 [X]
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/game/gen (class and option correspondence tables)
    used_for: the Sly 2 to Sly 3 correspondence of classes and options (rows scored 80 or more); marked [R]
reviewed_by: pending
date: 2026-11-19
---

# Sly 3 effects

Read [../sly2/effects.md](../sly2/effects.md) first. Layouts: [classes/effects-logic-ui.md](classes/effects-logic-ui.md)
1 and 2. Lights and the world shadow colour are in [rendering.md](rendering.md) 2. Tags as in [guards.md](guards.md).

## 1. Class map [D] [R]

| Sly 3 class | Sly 2 class | Role | Bodies |
|---:|---:|---|---:|
| 27 | 29 | continuous particle emitter | 543 |
| 30 | 32 | explosion part | 2 |
| 31 | 33 | explosion emitter | 100 |
| 29 | 31 | grouper (Sly 2's explosion group, [classes/paths-and-volumes.md](classes/paths-and-volumes.md) 7) | 112 |
| **45** | — | **effect object**: an option list and a list of components | 7172 |
| 191, 192 | — | effect components (run-time classes; only inside effects) | 15300, 2468 |
| 110 | 97 | sky (`o_sky_dome`, `h_skydome`, …) | 39, in 21 worlds |
| 4 | 4 | light | 2,348 |

The effect object is new in Sly 3: most particle effects (7,172 effects, 17,768 components) are effects, not Sly 2
emitters. Sly 2's emitter behaviour (emission, origin, launch and motion, drawing; Sly 2 1.2 to 1.6) applies to the
emitter classes [R]. The components carry options 552 to 603, which have no Sly 2 correspondence [R]; their
meanings are open, except 558, the component's **position kind** (value 3: an emission mesh follows the options,
[classes/effects-logic-ui.md](classes/effects-logic-ui.md) 2) [X] [D].

## 2. Continuous emitters (class 27)

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 [R] |
|---:|---|---:|---|---:|
| 609 | float pair at +0xE8 | 543 | (20,20) ×83; (15,20) ×73; (20,30) ×57 | 472 |
| 630 | float pair at +0x50 | 543 | (0,0) ×153; (500,1000) ×45; (0,200) ×36 | 496 |
| 632 | vector at +0x60 | 543 | (0,0,0) ×310; (0,0,30) ×45; (0,0,100) ×35 | 498 |
| 633 | float at +0x58 | 543 | -1 ×143; -3 ×124; 0 ×82 | — |
| 639 | float at +0x8C | 543 | 0.3 ×124; 0.25 ×76; 0.6 ×47 | — |
| 642 | type 0x9 at +0xB0 | 543 | (0.5,0,-0.5) ×63; (1,0,0) ×52; (0,4,-4) ×36 | — |
| 660 | object at +0x10 | 542 | 39760 ×54; 11114 ×36; 11115 ×36 | 526 |
| 663 | type 0x9 at +0x20 | 542 | (5,0,-4) ×52; (8,-3,0) ×36; (60,0,0) ×36 | — |
| 636 | float pair at +0x90 | 528 | (0,3.14) ×200; (0,0) ×121; (0,3) ×54 | 502 |
| 629 | type 0x1020 at +0x18 | 507 | 2 ×305; 0 ×101; 1 ×101 | — |
| 13 | setter | 417 | 3000 ×227; 2000 ×72; 2500 ×62 | 13 |
| 625 | vector at +0x40 | 395 | (1,0,0) ×271; (0,0,1) ×106; (-1,0,0) ×17 | — |
| 617 | setter | 314 | 1 ×278; 3 ×36 | — |
| 631 | float at +0x5C | 312 | 0 ×171; 1 ×96; 0.5 ×45 | 497 |
| 619 | vector at +0x0 | 278 | (-36.1818,-36.1818,-12.9374) ×36; (-37.0039,-34.2136,-11.8267) ×36; (-32.9963,-31.7192,-60.0175) ×27 | — |
| 620 | vector at +0x10 | 278 | (36.1818,36.1818,12.9374) ×36; (37.0039,34.2136,11.8267) ×36; (23.306,31.7986,76.8968) ×27 | — |
| 657 | setter | 278 | 2 ×218; 1 ×60 | — |
| 638 | float at +0x88 | 239 | 1 ×211; 0 ×28 | — |
| 666 | float pair at +0x8 | 218 | (-15,15) ×72; (-2,2) ×58; (-1,1) ×40 | — |
| 641 | float pair at +0xA0 | 190 | (-0.2,0.2) ×83; (0.3,0.5) ×36; (-0.5,0.5) ×22 | — |
| 27 | method (0 args) | 175 | () ×175 | 26 |
| 665 | bool at +0x4 | 168 | 1 ×168 | — |
| 662 | float at +0x18 | 165 | 0.5 ×63; 0.3 ×36; 0.2 ×35 | — |
| 659 | bool at +0x8 | 157 | 1 ×121; 0 ×36 | — |
| 664 | type 0x1024 at +0x0 | 101 | 1 ×101 | — |
| 611 | object at +0xF8 | 82 | 14517 ×36; 14518 ×36; 14030 ×10 | 477 |
| 628 | float pair at +0x24 | 75 | (0,0) ×63; (-10,10) ×12 | 494 |
| 667 | float at +0x0 | 60 | 5 ×36; 10 ×24 | — |
| 624 | type 0x101F at +0x14 | 36 | 5 ×36 | — |
| 634 | float at +0x80 | 36 | 5 ×36 | — |
| 635 | vector at +0x70 | 36 | (0,0,1) ×36 | — |
| 640 | float pair at +0x98 | 21 | (0,1) ×16; (0.1,0.2) ×2; (0,0.1) ×1 | — |
| 616 | setter | 11 | 0 ×10; 1 ×1 | — |
| 621 | object at +0x20 | 10 | 7104 ×10 | 487 |
| 658 | type 0x1022 at +0x4 | 2 | 2 ×2 | — |
| 661 | object at +0x14 | 2 | 84340 ×1; 84342 ×1 | 527 |
| 643 | setter | 1 | 2 ×1 | — |
| 644 | type 0x1027 at +0x20 | 1 | 1 ×1 | — |
| 645 | object at +0xC | 1 | 143447 ×1 | — |
| 649 | float pair at +0x14 | 1 | (0.6,1) ×1 | — |

617 is the emitter's position kind (3: an emission mesh follows; 36 emitters) [X] [D]. Options with a Sly 2
counterpart keep Sly 2's meaning (Sly 2 1.2 to 1.5) [R]:

- 609 (472) emission rate range;
- 611 (477) emit together with this group;
- 621 (487) the object whose surface emits;
- 628 (494) start offset along the direction;
- 630 (496) launch speed range, 631 (497) vertical speed factor, 632 (498) acceleration;
- 636 (502) tilt range;
- 660 (526) the sprites' shader, 661 (527) a reference object.

The other options have no correspondence; their meanings are open.

## 3. Explosion emitters (class 31) and parts (class 30)

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 [R] |
|---:|---|---:|---|---:|
| 692 | vector at +0x60 | 115 | (0,0,-2000) ×57; (0,0,200) ×18; (0,0,100) ×12 | — |
| 682 | float pair at +0x50 | 100 | (500,1500) ×57; (75,100) ×12; (300,600) ×9 | 548 |
| 684 | float pair at +0x90 | 100 | (0,0.8) ×57; (0,3.14) ×30; (0.5,1.5) ×12 | 550 |
| 689 | float pair at +0xE4 | 100 | (15,15) ×49; (5,5) ×16; (1,1) ×12 | — |
| 693 | float at +0x58 | 100 | -2 ×57; -5 ×17; -2.2 ×12 | 559 |
| 696 | float at +0x8C | 100 | 0.5 ×65; 0.2 ×12; 0.4 ×9 | 562 |
| 698 | type 0x9 at +0xB0 | 100 | (1,0,-0.5) ×37; (0.8,0,-0.5) ×16; (0.6,0,-0.6) ×9 | 564 |
| 715 | object at +0x10 | 99 | 39760 ×9; 75475 ×6; 75479 ×4 | 581 |
| 718 | type 0x9 at +0x20 | 99 | (5,0,-3) ×52; (10,5,-5) ×12; (30,0,10) ×9 | — |
| 681 | type 0x1020 at +0x18 | 74 | 1 ×58; 2 ×16 | — |
| 712 | setter | 67 | 1 ×65; 2 ×2 | — |
| 722 | float at +0x0 | 65 | 3 ×57; 10 ×8 | — |
| 723 | type 0xE at +0x0 | 37 | 595 ×14; 600 ×13; 601 ×4 | 589 |
| 724 | float at +0x8 | 37 | 3000 ×14; 5000 ×10; 6000 ×10 | 590 |
| 725 | float at +0x4 | 37 | 300 ×20; 3000 ×14; 1500 ×3 | 591 |
| 726 | float at +0xC | 36 | 1 ×28; 0.7 ×3; 0.6 ×3 | 592 |
| 727 | float at +0x10 | 36 | 0 ×17; -0.8 ×12; 0.1 ×3 | 593 |
| 686 | float at +0x88 | 31 | 1 ×21; 0 ×10 | — |
| 697 | float pair at +0xA0 | 25 | (0,0.3) ×12; (0.3,0.5) ×8; (-0.3,0.3) ×5 | 563 |
| 13 | setter | 24 | 3000 ×24 | 13 |
| 677 | vector at +0x40 | 16 | (0,0,0) ×8; (0,0,1) ×8 | — |
| 690 | bool at +0xEC | 12 | 1 ×12 | 556 |
| 714 | bool at +0x8 | 11 | 1 ×11 | — |
| 670 | setter | 10 | 1 ×10 | — |
| 672 | vector at +0x0 | 10 | (-32.9641,-28.8585,-34.7048) ×9; (-30.5144,-86.0668,-60.9055) ×1 | 538 |
| 673 | vector at +0x10 | 10 | (23.2808,28.9044,44.4623) ×9; (24.3764,94.6706,76.8383) ×1 | 539 |
| 683 | float at +0x5C | 8 | 1 ×5; 0 ×2; 3 ×1 | 549 |
| 694 | float at +0x80 | 5 | 2 ×5 | 560 |
| 680 | float pair at +0x24 | 2 | (-2,2) ×2 | 546 |
| 719 | type 0x1024 at +0x0 | 2 | 0 ×2 | — |
| 721 | float pair at +0x8 | 2 | (-2,2) ×2 | — |
| 688 | setter | 1 | 7 ×1 | — |
| 691 | setter | 1 | 1 ×1 | — |
| 699 | type 0x1027 at +0x20 | 1 | 1 ×1 | — |
| 700 | object at +0xC | 1 | 5167 ×1 | 568 |
| 704 | float pair at +0x14 | 1 | (0.6,1.5) ×1 | 570 |

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 [R] |
|---:|---|---:|---|---:|
| 681 | type 0x1020 at +0x18 | 2 | 0 ×2 | — |
| 682 | float pair at +0x50 | 2 | (100,100) ×2 | 548 |
| 683 | float at +0x5C | 2 | 1 ×2 | 549 |
| 684 | float pair at +0x90 | 2 | (0,3.14) ×2 | 550 |
| 686 | float at +0x88 | 2 | 1 ×2 | — |
| 687 | float pair at +0x98 | 2 | (-0.1,0.1) ×2 | — |

## 4. Effect objects (class 45)

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 [R] |
|---:|---|---:|---|---:|
| 539 | type 0x101C at +0xD8 | 7047 | 0 ×3326; 1 ×2650; 2 ×1063 | — |
| 542 | object of class 2 at +0xE4 | 4685 | -1 ×4685 | — |
| 13 | setter | 2607 | 3000 ×1541; 4000 ×309; 5000 ×198 | 13 |
| 27 | method (0 args) | 957 | () ×957 | 26 |
| 541 | object of class 2 at +0xE0 | 23 | -1 ×22; 24 ×1 | — |

Components (class 191, 15300 bodies):

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 [R] |
|---:|---|---:|---|---:|
| 584 | object at +0x0 | 15293 | 11126 ×144; 10595 ×124; 48335 ×112 | — |
| 580 | type 0x9 at +0x30 | 15265 | (0.7,0,0) ×1100; (1,0,0) ×1075; (0,1,-1) ×771 | — |
| 581 | type 0x9 at +0x20 | 15265 | (10,0,-10) ×550; (5,0,-5) ×433; (4,0,-4) ×341 | — |
| 569 | float pair at +0x40 | 15264 | (0,0) ×2557; (100,200) ×580; (400,800) ×578 | — |
| 571 | float at +0x68 | 15264 | -1 ×5509; -2 ×2030; -3 ×1775 | — |
| 572 | vector at +0x0 | 15264 | (0,0,0) ×7354; (0,0,-300) ×1031; (0,0,100) ×951 | — |
| 577 | float at +0x78 | 15264 | 0.5 ×2152; 0.3 ×1709; 0.4 ×1660 | — |
| 575 | float pair at +0x48 | 15155 | (0,3.14) ×7580; (0,0) ×2401; (0,2) ×990 | — |
| 567 | type 0x1020 at +0x60 | 15057 | 2 ×10283; 0 ×4479; 1 ×295 | — |
| 570 | float at +0x6C | 14701 | 1 ×13129; 0 ×909; 1.5 ×393 | — |
| 564 | vector at +0x30 | 14670 | (0,0,1) ×10540; (1,0,0) ×3007; (0,0,-1) ×726 | — |
| 576 | float at +0x74 | 14588 | 1 ×12927; 0 ×1657; 0.5 ×4 | — |
| 558 | setter | 11982 | 1 ×11925; 3 ×56; 0 ×1 | — |
| 560 | vector at +0x0 | 11926 | (-2.5,-2.5,-2.5) ×1796; (-15,-15,-15) ×1566; (-25,-25,-25) ×836 | — |
| 561 | vector at +0x10 | 11926 | (2.5,2.5,2.5) ×1796; (15,15,15) ×1566; (25,25,25) ×836 | — |
| 582 | setter | 11840 | 2 ×9064; 1 ×2773; 0 ×3 | — |
| 579 | float pair at +0x58 | 11313 | (0.3,0.5) ×1789; (0,0.2) ×1407; (-0.05,0.05) ×1403 | — |
| 592 | float pair at +0x8 | 9086 | (-2,2) ×4118; (-1,1) ×1906; (-3,3) ×1138 | — |
| 591 | bool at +0x4 | 8649 | 1 ×7358; 0 ×1291 | — |
| 553 | method (2 args) | 7644 | ((30, 35), -1) ×653; ((25, 30), -1) ×617; ((20, 25), -1) ×599 | — |
| 552 | method (1 args) | 7538 | ((25, 30)) ×744; ((20, 25)) ×723; ((35, 40)) ×701 | — |
| 587 | method (8 args) | 5632 | (150, 150, 150, 255, -1, -1, -1, 255) ×844; (255, 200, 0, 255, -1, -1, -1, 255) ×395; (200, 0, 0, 255, -1, -1, -1, 255) ×238 | — |
| 554 | method (5 args) | 2792 | (198, 500, 5000, 0.7, 0) ×108; (171, 500, 5000, 1, 0) ×94; (176, 1000, 20000, 1, 0.5) ×68 | — |
| 593 | float at +0x0 | 2751 | 5 ×454; 13 ×425; 10 ×367 | — |
| 586 | float at +0x1C | 2713 | 20 ×1514; 0.3 ×246; 50 ×212 | — |
| 578 | float pair at +0x50 | 2668 | (0,0.1) ×1822; (0,0.03) ×175; (0,0.2) ×149 | — |
| 585 | bool at +0x14 | 2596 | 1 ×2503; 0 ×93 | — |
| 557 | method (1 args) | 1602 | (-1) ×1602 | — |
| 588 | bool at +0x18 | 704 | 1 ×704 | — |
| 573 | float at +0x70 | 655 | 2 ×174; 1 ×163; 3 ×95 | — |
| 556 | method (1 args) | 436 | ((0.2, 0.2)) ×95; ((0.6, 1)) ×56; ((0.3, 1.5)) ×39 | — |
| 590 | type 0x1024 at +0x0 | 265 | 1 ×265 | — |
| 555 | method (0 args) | 260 | () ×260 | — |
| 563 | type 0x101F at +0x4C | 241 | 1 ×124; 4 ×78; 0 ×39 | — |
| 566 | bool at +0x7C | 197 | 1 ×197 | — |
| 562 | object at +0x0 | 3 | 88869 ×2; -1 ×1 | — |

Components (class 192, 2468 bodies):

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 [R] |
|---:|---|---:|---|---:|
| 596 | object at +0xC | 2466 | 4751 ×266; 9000 ×102; 25167 ×88 | — |
| 571 | float at +0x68 | 2455 | -1 ×1864; -2 ×94; 0 ×92 | — |
| 575 | float pair at +0x48 | 2451 | (0,2) ×602; (0,3.14) ×555; (0,0) ×474 | — |
| 577 | float at +0x78 | 2450 | 1.5 ×1027; 2 ×337; 0.3 ×161 | — |
| 569 | float pair at +0x40 | 2449 | (0,0) ×501; (300,700) ×256; (300,600) ×184 | — |
| 580 | type 0x9 at +0x30 | 2449 | (1,0,-1) ×1691; (4,-4,0) ×101; (0.6,0,-0.6) ×95 | — |
| 598 | float pair at +0x14 | 2448 | (1,1) ×349; (0.7,1.2) ×166; (0.7,1) ×123 | — |
| 564 | vector at +0x30 | 2447 | (0,0,1) ×1966; (1,0,0) ×409; (-0.5,0,0.5) ×25 | — |
| 572 | vector at +0x0 | 2444 | (0,0,-1500) ×1320; (0,0,0) ×504; (0,0,-1000) ×186 | — |
| 594 | setter | 2434 | 7 ×1759; 2 ×430; 4 ×142 | — |
| 570 | float at +0x6C | 2403 | 1 ×1671; 1.5 ×380; 2 ×205 | — |
| 567 | type 0x1020 at +0x60 | 2359 | 2 ×1854; 0 ×460; 1 ×45 | — |
| 576 | float at +0x74 | 2337 | 1 ×2290; 0 ×39; 0.2 ×8 | — |
| 558 | setter | 2321 | 1 ×2316; 3 ×5 | — |
| 560 | vector at +0x0 | 2316 | (-2.5,-2.5,-2.5) ×146; (-50,-50,-50) ×107; (-51.4551,-138.856,266.487) ×72 | — |
| 561 | vector at +0x10 | 2316 | (2.5,2.5,2.5) ×146; (50,50,50) ×107; (54.1029,126.957,300.817) ×72 | — |
| 581 | type 0x9 at +0x20 | 2264 | (0,0,0) ×1345; (0,0.5,-0.5) ×138; (0,1,-2) ×70 | — |
| 552 | method (1 args) | 2234 | ((2, 2)) ×341; ((1, 1)) ×304; ((8, 10)) ×144 | — |
| 554 | method (5 args) | 1514 | (40, 500, 5000, 1, -1) ×124; (40, 500, 5000, 1, -1.2) ×124; (159, 300, 3000, 1, 0) ×65 | — |
| 579 | float pair at +0x58 | 1182 | (-0.1,0.1) ×593; (-0.05,0.05) ×189; (-0.05,0) ×107 | — |
| 578 | float pair at +0x50 | 567 | (0,0.1) ×339; (0,0.2) ×161; (0.05,0.05) ×31 | — |
| 553 | method (2 args) | 361 | ((5, 6), -1) ×80; ((10, 15), -1) ×39; ((30, 35), -1) ×37 | — |
| 556 | method (1 args) | 235 | ((0.1, 0.1)) ×91; ((0.15, 0.15)) ×59; ((0.05, 0.05)) ×39 | — |
| 557 | method (1 args) | 105 | (-1) ×105 | — |
| 597 | object at +0x10 | 86 | 21000 ×41; 56397 ×6; 9508 ×6 | — |
| 603 | type 0x1026 at +0x0 | 29 | 0 ×29 | — |
| 565 | float pair at +0x40 | 14 | (30,30) ×14 | — |

## 5. Sky (class 110)

39 bodies in 21 worlds; the only option on the disc is 32 = 1 on 10 [D]. Behaviour: Sly 2 3 [R].

## 6. Open questions

1. The effect components' options (552 to 603) and how an effect object starts, runs and ends its components.
2. The emitter options without a Sly 2 correspondence.
3. Option 539 of the effect object (0, 1, 2 on most): *interpretation:* a trigger or loop mode [I].
