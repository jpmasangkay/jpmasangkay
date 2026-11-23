use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use profile_metrics::svg::Theme;
use profile_metrics::{github, svg};

const DEFAULT_USERNAME: &str = "jpmasangkay";
const DEFAULT_TZ_OFFSET: i32 = 8;
const DEFAULT_DARK_OUTPUT: &str = "assets/metrics-dark.svg";
const DEFAULT_LIGHT_OUTPUT: &str = "assets/metrics-light.svg";

struct CliArgs {
    mock: bool,
    username: String,
    tz_offset: i32,
    dark_output: PathBuf,
    light_output: PathBuf,
}

fn parse_args() -> Result<CliArgs> {
    let mut mock = false;
    let mut username = DEFAULT_USERNAME.to_string();
    let mut tz_offset = DEFAULT_TZ_OFFSET;
    let mut dark_output = PathBuf::from(DEFAULT_DARK_OUTPUT);
    let mut light_output = PathBuf::from(DEFAULT_LIGHT_OUTPUT);

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mock" => mock = true,
            "--username" | "-u" => {
                username = args
                    .next()
                    .context("missing value for --username (expected GitHub login)")?;
            }
            "--tz-offset" => {
                let raw = args
                    .next()
                    .context("missing value for --tz-offset (expected integer hours, e.g. 8)")?;
                tz_offset = raw
                    .parse::<i32>()
                    .with_context(|| format!("invalid --tz-offset integer: {raw}"))?;
                if !(-12..=14).contains(&tz_offset) {
                    bail!("--tz-offset must be between -12 and 14, got {tz_offset}");
                }
            }
            "--dark-output" => {
                let value = args
                    .next()
                    .context("missing value for --dark-output (expected file path)")?;
                dark_output = PathBuf::from(value);
            }
            "--light-output" => {
                let value = args
                    .next()
                    .context("missing value for --light-output (expected file path)")?;
                light_output = PathBuf::from(value);
            }
            "--help" | "-h" => {
                println!(
                    "Usage: profile-metrics [--mock] [--username LOGIN] [--tz-offset HOURS] [--dark-output PATH] [--light-output PATH]\n\
                     \n\
                     Options:\n\
                       --mock                 Use offline mock data for layout preview\n\
                       --username LOGIN       GitHub username to query (default: {DEFAULT_USERNAME})\n\
                       --tz-offset HOURS      UTC offset for coding habit charts (default: {DEFAULT_TZ_OFFSET})\n\
                       --dark-output PATH     Dark mode SVG output path (default: {DEFAULT_DARK_OUTPUT})\n\
                       --light-output PATH    Light mode SVG output path (default: {DEFAULT_LIGHT_OUTPUT})"
                );
                std::process::exit(0);
            }
            other => bail!("unknown argument: {other} (run with --help for usage)"),
        }
    }

    Ok(CliArgs {
        mock,
        username,
        tz_offset,
        dark_output,
        light_output,
    })
}

fn write_svg(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory: {}", parent.display()))?;
        }
    }
    fs::write(path, content)
        .with_context(|| format!("failed to write SVG output: {}", path.display()))?;
    Ok(())
}

fn main() -> Result<()> {
    let args = parse_args()?;

    let data = if args.mock {
        github::mock_metrics(&args.username, args.tz_offset)
    } else {
        let token = env::var("GITHUB_TOKEN").context(
            "GITHUB_TOKEN environment variable is required (or pass --mock for offline preview)",
        )?;
        github::fetch_live_metrics(&token, &args.username, args.tz_offset)?
    };

    github::print_repo_size_diagnostics(&data.top_public_repos_by_bytes);

    let dark_svg = svg::render_metrics_svg(&data, Theme::Dark);
    let light_svg = svg::render_metrics_svg(&data, Theme::Light);

    write_svg(&args.dark_output, &dark_svg)?;
    write_svg(&args.light_output, &light_svg)?;

    println!(
        "Generated {} and {}",
        args.dark_output.display(),
        args.light_output.display()
    );
    Ok(())
}
