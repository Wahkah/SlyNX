---
id: sly3-classes-simple-layouts
title: Sly 3 simple layouts
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97464 image, all 39 worlds
    used_for: every body of these classes parsed to its declared size
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly3-Release loaders (option-list, header, mission and task loaders)
    used_for: first reading of the field order; re-checked by the parse
reviewed_by: pending
date: 2026-11-14
---

# Simple layouts (Sly 3)

Small layouts, each confirmed by parsing every body of its classes to the declared size. [D] Class counts:
[../object-stream.md](../object-stream.md) 3.2. Tags as in [../disc.md](../disc.md).

## 1. Options

The body is one option list. Classes 2, 133, 134, 136, 137, 138, 139, 141, 142, 144, 145, 147 to 150, 152, 153, 154,
156. The shortest body is `FF FF`. [D]

## 2. Header + options

| Order | Field | Type |
|---:|---|---|
| 1 | parent | `s16` transform reference |
| 2 | matrix | 48 bytes |
| 3 | zone mask | `u32` |
| 4 | options | option list |

As Sly 2 2. Classes 3 to 6, 15, 17, 18, 33 to 35, 37, 38, 41 to 44, 46. [D] Example: a class-3 body of 56 bytes
(parent, matrix, zone mask, an empty list). [D]

## 3. Header + options + name

Section 2, then a `u16`-length string. Classes 39 and 40. [D]

## 4. Options + name

An option list, then a `u16`-length string. Classes 168 and 170. [D]

## 5. Task

| Order | Field | Type |
|---:|---|---|
| 1 | options | option list |
| 2 | list A | `u8` count, then count × `s32` |
| 3 | list B | `u8` count, then count × `s32` |
| 4 | name | `u16`-length string |

Class 169 (5,519 bodies, the jobs and tasks of the macro groups). [D] *Interpretation:* lists of related task ids. [I]
