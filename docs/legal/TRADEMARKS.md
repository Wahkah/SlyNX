# Trademarks and the project name

Status: draft, 2026-11-13. Not legal advice. A trademark clearance search and a lawyer's view are needed before the
project gets real visibility **[lawyer]**. Items marked **[verify online]** were not checked against the registers.

## 1. Whose marks are involved

- **Sly Cooper**, the game titles (*Sly Cooper and the Thievius Raccoonus*, *Sly 2: Band of Thieves*, *Sly 3: Honor
  Among Thieves*), the logos and the characters (Sly, Bentley, Murray, Carmelita and the rest) belong to Sony
  Interactive Entertainment; the games were developed by Sucker Punch Productions. Characters are protected by
  copyright as well as trademark. **[verify online]** the exact registrations (USPTO TESS / EUIPO eSearch for "SLY
  COOPER" and "SLY").
- **Nintendo Switch**, the Switch logo and Joy-Con are Nintendo's. "NX" was the public codename of the Switch before
  launch; it is widely used in homebrew names (nxdumptool, NX-Shell and others) and is low risk on its own.
- **PlayStation** and the PS2 marks are Sony's.

## 2. Risk assessment of "SlyNX"

| Factor | Assessment |
|---|---|
| Shares the dominant word | "Sly" is the first and distinctive word of "Sly Cooper", and the games are commonly called "Sly 2", "Sly 3". |
| Same goods | Video game software, the class the marks are registered in. Direct overlap. |
| Suggests affiliation? | "SlyNX" reads as "Sly for Nintendo Switch", i.e. like an official Sly product on a Nintendo platform. That is the main concern: it does not just identify the games, it brands our product with their mark. |
| Nominative fair use | Using "Sly Cooper" to say *which games* the tool works with is the classic nominative use (*New Kids on the Block v. News America*, 9th Cir. 1992: use only as much of the mark as needed, no suggestion of sponsorship). Building the mark into our own product name goes beyond that. |
| Commercial use | None (free, open source). This lowers the practical risk but is not a defence on its own. |
| Enforcement context | Rights holders tend to act against fan projects when there is money, piracy, or confusion with an official release. A Switch port of a Sony game touches all three perceptions even when it ships no game data. Sony's enforcement history toward Sly fan projects is unknown to us. **[verify online]** |

**Overall: moderate risk.** The likely worst case is a trademark complaint to GitHub forcing a rename, which costs
more the later it happens (links, forks, issue history, package names). The copyright side (no game data, clean-room
code) matters more for a takedown than the name, but a name that sounds official draws attention to everything else.

If the owner keeps "SlyNX": always pair it with the disclaimer (section 4), never use Sony's logo style or fonts
for it, and be ready to rename quickly.

## 3. How similar projects handle it

From memory of public projects; **[verify online]** before citing any of these externally.

- **Avoid the mark entirely:** *Ship of Harkinian* (Zelda: Ocarina of Time PC port, named after a character joke, not
  "Zelda"), *2 Ship 2 Harkinian* (Majora's Mask), *Starship* (Star Fox 64), *SafeHouse* (Sly 1/2/3 on PC),
  *ProjectCane* (Sly 1 on PC), *OpenThievius* (Sly decompilations; uses a word from a game subtitle, not the main
  mark).
- **Use the mark descriptively with an "Open"/engine prefix:** OpenMW (Morrowind), OpenRCT2, OpenGOAL (named after
  the Jak and Daxter engine's language, not "Jak").
- **Use the mark directly:** many smaller ports and decompilations (for example "sm64-port", "sly2-decomp",
  "Sly-2-Recomp", "sly-rs"). Common for repositories that are explicitly about the game, and some of them have
  been taken down, usually over shipped game code or data rather than the name.

The pattern among the larger, longer-lived fan ports is an original name plus a plain sentence saying which game it
runs, and the official name only in the description.

## 4. Disclaimer (use in README, release notes and the packager's about screen)

> SlyNX is an unofficial fan project. It is not affiliated with, endorsed by or sponsored by Sony Interactive
> Entertainment, Sucker Punch Productions or Nintendo. Sly Cooper and related names and characters are trademarks of
> Sony Interactive Entertainment. Nintendo Switch is a trademark of Nintendo. These names are used only to identify
> the games and the platform. No game data is included; you need your own copy of the games.

Short form for tight places (NACP description, packager window title bar tooltip): "Unofficial; not affiliated with
Sony, Sucker Punch or Nintendo."

## 5. Rules for names, icons and artwork

1. **No official logos, box art, screenshots of official artwork, fonts styled after the logos, or character art in
   the repository**, the README, the GitHub social preview, releases or the wiki.
2. **Personal cover art or icons that depict the characters stay out of git**, whoever made them: they show the
   trademarked (and copyrighted) characters. Keep them in a folder outside the repository and pass it to the packager by path. The release
   audit should block image files under any tracked `art`/`icons` folder that are not on the approved list.
3. **The project's own logo and default icon must be original and neutral:** no characters, no raccoon in Sly's
   style, no cane in the game's style, nothing a viewer would mistake for official art. A plain wordmark or abstract
   symbol is safest.
4. **Player builds:** the packager may let players choose their own icon and title for builds they make for
   themselves (see [DISTRIBUTION.md](DISTRIBUTION.md)). The defaults it ships with are the neutral project icon
   and a title such as "SlyNX (Sly 1)".
5. **Game titles in text** are fine to identify compatibility ("works with your own disc of *Sly 2: Band of
   Thieves*"), in plain text, without the logo.
6. **No Sony or Nintendo name in a domain, organisation name or social account** for the project.
7. Screenshots of our build running with the player's own data show copyrighted game content. Avoid them in the
   repository; a few in an issue or a release note are a common practice that is usually tolerated but not risk-free.
   **[lawyer]** if they are used for promotion.

## 6. Lower-risk alternative names

None of these were checked against trademark registers or existing projects **[verify online]**: search USPTO,
EUIPO, GitHub and crates.io before choosing.

| Name | Why |
|---|---|
| **Ringtail** | Generic word for a raccoon; evokes the series without using any mark. |
| **Pickpocket** | Thief theme, generic word. (Avoid anything with "Lockpick": a Switch key-dumping tool has that name, and the association would hurt.) |
| **Moonheist** | Original coinage; night-time heist theme. |
| **Caper** | Generic word for a heist; short, crate-friendly (check crates.io). |
| **NightshiftNX** | Keeps the "NX" platform hint; an original phrase rather than a game mark. |

If the owner wants continuity with the current name, "SlyNX" could stay as an internal codename while the public
repository uses one of these, with the description line "a native port engine for the Sly Cooper trilogy".
