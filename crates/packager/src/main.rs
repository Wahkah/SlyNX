//! `slynx-pack`: packages the player's own disc image with SlyNX for the Switch. See `docs/packager.md`.

use packager::game::identify_path;
use packager::pack::{Layout, PERSONAL_USE, PackOptions, inspect_nro, pack};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
usage:
  slynx-pack <game.iso> --out <dir> [options]   package your own disc image
  slynx-pack --identify <game.iso>              say which disc this is
  slynx-pack --inspect <file.nro>               check a package and show what it holds
  slynx-pack --write-icon <out.jpg> [--number N]  write the default icon (N = 1, 2, 3 or 0)

options:
  --out <dir>          output folder; the package goes to <dir>/switch/SlyNX/ (copy that folder to the SD card)
  --program <file>     the SlyNX program (.elf or .nro without game data); default: slynx-game.nro or
                       slynx-game.elf next to slynx-pack
  --icon <file.jpg>    your own 256x256 JPEG icon (default: the SlyNX icon)
  --title <text>       title in the Homebrew Menu (default: \"SlyNX 1\" for Sly 1)
  --layout <mode>      auto (default): the disc image goes inside the .nro if the result stays under 4 GB,
                       otherwise next to it; romfs: always inside; sd: always next to it
  --nsp --keys <file>  NSP output with your own keys: not supported yet (the key file is never read)
";

struct Args {
    iso: Option<PathBuf>,
    out: Option<PathBuf>,
    program: Option<PathBuf>,
    icon: Option<PathBuf>,
    title: Option<String>,
    layout: Layout,
    nsp: bool,
    keys: Option<PathBuf>,
    identify: Option<PathBuf>,
    inspect: Option<PathBuf>,
    write_icon: Option<PathBuf>,
    number: u8,
}

fn parse(args: Vec<String>) -> Result<Args, String> {
    let mut a = Args {
        iso: None,
        out: None,
        program: None,
        icon: None,
        title: None,
        layout: Layout::Auto,
        nsp: false,
        keys: None,
        identify: None,
        inspect: None,
        write_icon: None,
        number: 0,
    };
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "-h" | "--help" => return Err(String::new()),
            "--out" | "-o" => a.out = Some(value("--out")?.into()),
            "--program" => a.program = Some(value("--program")?.into()),
            "--icon" => a.icon = Some(value("--icon")?.into()),
            "--title" => a.title = Some(value("--title")?),
            "--layout" => {
                a.layout = match value("--layout")?.as_str() {
                    "auto" => Layout::Auto,
                    "romfs" => Layout::RomFs,
                    "sd" => Layout::Sd,
                    other => return Err(format!("unknown layout {other:?} (auto, romfs or sd)")),
                }
            }
            "--nsp" => a.nsp = true,
            "--keys" => a.keys = Some(value("--keys")?.into()),
            "--identify" => a.identify = Some(value("--identify")?.into()),
            "--inspect" => a.inspect = Some(value("--inspect")?.into()),
            "--write-icon" => a.write_icon = Some(value("--write-icon")?.into()),
            "--number" => {
                a.number = value("--number")?.parse().ok().filter(|n| *n <= 3).ok_or("--number takes 0 to 3")?
            }
            s if s.starts_with('-') => return Err(format!("unknown option {s}")),
            s => {
                if a.iso.replace(s.into()).is_some() {
                    return Err("only one disc image, please".into());
                }
            }
        }
    }
    Ok(a)
}

fn default_program() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().unwrap_or(Path::new("."));
    for name in ["slynx-game.nro", "slynx-game.elf"] {
        let p = dir.join(name);
        if p.is_file() {
            return Ok(p);
        }
    }
    Err(format!(
        "no SlyNX program found next to slynx-pack (looked for slynx-game.nro and slynx-game.elf in {}); \
         pass one with --program",
        dir.display()
    ))
}

fn run(a: Args) -> Result<(), String> {
    if let Some(path) = a.write_icon {
        std::fs::write(&path, packager::icon::default_icon(a.number))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        println!("wrote {}", path.display());
        return Ok(());
    }
    if let Some(path) = a.identify {
        println!("{}: {}", path.display(), identify_path(&path)?);
        return Ok(());
    }
    if let Some(path) = a.inspect {
        let info = inspect_nro(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("{}: valid .nro", path.display());
        println!("  title {:?}, author {:?}, version {:?}", info.nacp.title, info.nacp.author, info.nacp.version);
        println!("  program image {:#x} bytes, icon {}x{}", info.header.size, info.icon_size.0, info.icon_size.1);
        println!("  RomFS {} bytes:", info.assets.romfs.1);
        if let Some(r) = &info.romfs {
            for f in &info.romfs_files {
                println!("    {f} ({} bytes)", r.lookup(f).map_or(0, |s| s.size));
            }
        }
        return Ok(());
    }

    if a.nsp || a.keys.is_some() {
        // Refused before anything else is looked at; the key file is never opened.
        return Err(packager::pack::PackError::NspNotSupported.to_string());
    }
    let iso = a.iso.ok_or("no disc image given")?;
    let out = a.out.ok_or("no output folder given (--out)")?;
    let program = match a.program {
        Some(p) => p,
        None => default_program()?,
    };
    let mut opts = PackOptions::new(iso, out, program);
    opts.icon = a.icon;
    opts.title = a.title;
    opts.layout = a.layout;

    let mut last = u64::MAX;
    let report = pack(&opts, |done, total| {
        let pct = (done * 100).checked_div(total).unwrap_or(100);
        if pct != last {
            last = pct;
            eprint!("\r  copying disc image: {pct:3}%");
            let _ = std::io::stderr().flush();
        }
    })
    .map_err(|e| e.to_string())?;
    eprintln!();
    println!("{}", report.release);
    println!("wrote {} ({} bytes, title {:?})", report.nro.display(), report.nro_size, report.title);
    if let Some(img) = &report.image_on_sd {
        println!("wrote {} (the disc image did not fit in the .nro; keep it next to it)", img.display());
    }
    println!("Copy the switch/SlyNX folder from the output folder to the root of your SD card.");
    println!();
    println!("{PERSONAL_USE}");
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprint!("{USAGE}");
        return ExitCode::from(2);
    }
    let parsed = match parse(args) {
        Ok(a) => a,
        Err(e) if e.is_empty() => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("slynx-pack: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(parsed) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("slynx-pack: {e}");
            ExitCode::FAILURE
        }
    }
}
