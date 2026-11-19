use std::fmt::Write as _;

use chrono::{DateTime, Duration, Utc};

use crate::github::{ActiveRepo, ContributionDay, LanguageShare, MetricsData};

const WIDTH: u32 = 920;
const HEIGHT: u32 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy)]
struct Palette {
    text_primary: &'static str,
    text_muted: &'static str,
    accent_header: &'static str,
    bar_fill: &'static str,
    track_fill: &'static str,
    axis_line: &'static str,
    // 5 contribution levels (0..=4), each as (top_face, left_face, right_face)
    iso_levels: [(&'static str, &'static str, &'static str); 5],
}

impl Theme {
    fn palette(self) -> Palette {
        match self {
            Theme::Dark => Palette {
                text_primary: "#c9d1d9",
                text_muted: "#8b949e",
                accent_header: "#58a6ff",
                bar_fill: "#26a641",
                track_fill: "#21262d",
                axis_line: "#30363d",
                iso_levels: [
                    ("#161b22", "#12171d", "#0d1117"),
                    ("#0e4429", "#0a341f", "#072617"),
                    ("#006d32", "#005627", "#00421e"),
                    ("#26a641", "#1e8834", "#176b29"),
                    ("#39d353", "#2cb244", "#229136"),
                ],
            },
            Theme::Light => Palette {
                text_primary: "#24292f",
                text_muted: "#57606a",
                accent_header: "#0969da",
                bar_fill: "#2da44e",
                track_fill: "#eaeef2",
                axis_line: "#d0d7de",
                iso_levels: [
                    ("#ebedf0", "#d8dce1", "#c8cdd4"),
                    ("#9be9a8", "#81d690", "#6ac079"),
                    ("#40c463", "#33a952", "#288e43"),
                    ("#30a14e", "#26863f", "#1e6d32"),
                    ("#216e39", "#18562c", "#124221"),
                ],
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatRow {
    pub icon: &'static str,
    pub label: String,
}

// Inline 16x16 Octicon-style monochrome paths (zero external assets, zero emojis).
const ICON_PERSON: &str = "M10.561 8.073a6.005 6.005 0 0 1 3.432 5.142.75.75 0 1 1-1.498.07 4.5 4.5 0 0 0-8.99 0 .75.75 0 0 1-1.498-.07 6.004 6.004 0 0 1 3.431-5.142 3.999 3.999 0 1 1 5.123 0ZM10.5 5a2.5 2.5 0 1 0-5 0 2.5 2.5 0 0 0 5 0Z";
const ICON_CLOCK: &str = "M8 0a8 8 0 1 1 0 16A8 8 0 0 1 8 0ZM1.5 8a6.5 6.5 0 1 0 13 0 6.5 6.5 0 0 0-13 0Zm7-3.25v2.992l2.028.812a.75.75 0 0 1-.557 1.392l-2.5-1A.751.751 0 0 1 7 8.25v-3.5a.75.75 0 0 1 1.5 0Z";
const ICON_REPO: &str = "M2 2.5A2.5 2.5 0 0 1 4.5 0h8.75a.75.75 0 0 1 .75.75v12.5a.75.75 0 0 1-.75.75h-2.5a.75.75 0 0 1 0-1.5h1.75v-2h-8a1 1 0 0 0-.714 1.7.75.75 0 1 1-1.072 1.05A2.495 2.495 0 0 1 2 11.5Zm10.5-1h-8a1 1 0 0 0-1 1v6.708A2.486 2.486 0 0 1 4.5 9h8ZM5 12.25a.25.25 0 0 1 .25-.25h3.5a.25.25 0 0 1 .25.25v3.25a.25.25 0 0 1-.4.2l-1.45-1.087a.249.249 0 0 0-.3 0L5.4 15.7a.25.25 0 0 1-.4-.2Z";
const ICON_LAW: &str = "M8.75.75V2h.985c.304 0 .603.08.867.231l1.29.736c.038.022.08.033.124.033h2.234a.75.75 0 0 1 0 1.5h-.427l2.111 4.692a.75.75 0 0 1-.154.838l-.53-.53.529.531-.001.002-.002.002-.006.006-.006.005-.01.01-.045.04c-.21.176-.441.327-.686.45C14.556 10.78 13.88 11 13 11a4.498 4.498 0 0 1-2.023-.454 3.544 3.544 0 0 1-.686-.45l-.045-.04-.016-.015-.006-.006-.004-.004v-.001a.75.75 0 0 1-.154-.838L12.178 4.5h-.162c-.305 0-.604-.079-.868-.231l-1.29-.736a.245.245 0 0 0-.124-.033H8.75V13h2.5a.75.75 0 0 1 0 1.5h-6.5a.75.75 0 0 1 0-1.5h2.5V3.5h-.984a.245.245 0 0 0-.124.033l-1.289.737c-.265.15-.564.23-.869.23h-.162l2.112 4.692a.75.75 0 0 1-.154.838l-.53-.53.529.531-.001.002-.002.002-.006.006-.016.015-.045.04c-.21.176-.441.327-.686.45C4.556 10.78 3.88 11 3 11a4.498 4.498 0 0 1-2.023-.454 3.544 3.544 0 0 1-.686-.45l-.045-.04-.016-.015-.006-.006-.004-.004v-.001a.75.75 0 0 1-.154-.838L2.178 4.5H1.75a.75.75 0 0 1 0-1.5h2.234a.249.249 0 0 0 .125-.033l1.288-.737c.265-.15.564-.23.869-.23h.984V.75a.75.75 0 0 1 1.5 0Zm2.945 8.477c.285.135.718.273 1.305.273s1.02-.138 1.305-.273L13 6.327Zm-10 0c.285.135.718.273 1.305.273s1.02-.138 1.305-.273L3 6.327Z";
const ICON_TAG: &str = "M1 7.775V2.75C1 1.784 1.784 1 2.75 1h5.025c.464 0 .91.184 1.238.513l6.25 6.25a1.75 1.75 0 0 1 0 2.474l-5.026 5.026a1.75 1.75 0 0 1-2.474 0l-6.25-6.25A1.752 1.752 0 0 1 1 7.775Zm1.5 0c0 .066.026.13.073.177l6.25 6.25a.25.25 0 0 0 .354 0l5.025-5.025a.25.25 0 0 0 0-.354l-6.25-6.25a.25.25 0 0 0-.177-.073H2.75a.25.25 0 0 0-.25.25ZM6 5a1 1 0 1 1 0 2 1 1 0 0 1 0-2Z";
const ICON_DATABASE: &str = "M1 3.5c0-.626.292-1.165.7-1.59.406-.422.956-.767 1.579-1.041C4.525.32 6.195 0 8 0c1.805 0 3.475.32 4.722.869.622.274 1.172.62 1.578 1.04.408.426.7.965.7 1.591v9c0 .626-.292 1.165-.7 1.59-.406.422-.956.767-1.579 1.041C11.476 15.68 9.806 16 8 16c-1.805 0-3.475-.32-4.721-.869-.623-.274-1.173-.62-1.579-1.04-.408-.426-.7-.965-.7-1.591Zm1.5 0c0 .133.058.318.282.551.227.237.591.483 1.101.707C4.898 5.205 6.353 5.5 8 5.5c1.646 0 3.101-.295 4.118-.742.508-.224.873-.471 1.1-.708.224-.232.282-.417.282-.55 0-.133-.058-.318-.282-.551-.227-.237-.591-.483-1.101-.707C11.102 1.795 9.647 1.5 8 1.5c-1.646 0-3.101.295-4.118.742-.508.224-.873.471-1.1.708-.224.232-.282.417-.282.55Zm0 4.5c0 .133.058.318.282.551.227.237.591.483 1.101.707C4.898 9.705 6.353 10 8 10c1.646 0 3.101-.295 4.118-.742.508-.224.873-.471 1.1-.708.224-.232.282-.417.282-.55V5.724c-.241.15-.503.286-.778.407C11.475 6.68 9.805 7 8 7c-1.805 0-3.475-.32-4.721-.869a6.15 6.15 0 0 1-.779-.407Zm0 2.225V12.5c0 .133.058.318.282.55.227.237.592.484 1.1.708 1.016.447 2.471.742 4.118.742 1.647 0 3.102-.295 4.117-.742.51-.224.874-.47 1.101-.707.224-.233.282-.418.282-.551v-2.275c-.241.15-.503.285-.778.406-1.247.549-2.917.869-4.722.869-1.805 0-3.475-.32-4.721-.869a6.327 6.327 0 0 1-.779-.406Z";
const ICON_STAR: &str = "M8 .25a.75.75 0 0 1 .673.418l1.882 3.815 4.21.612a.75.75 0 0 1 .416 1.279l-3.046 2.97.719 4.192a.751.751 0 0 1-1.088.791L8 12.347l-3.766 1.98a.75.75 0 0 1-1.088-.79l.72-4.194L.818 6.374a.75.75 0 0 1 .416-1.28l4.21-.611L7.327.668A.75.75 0 0 1 8 .25Zm0 2.445L6.615 5.5a.75.75 0 0 1-.564.41l-3.097.45 2.24 2.184a.75.75 0 0 1 .216.664l-.528 3.084 2.769-1.456a.75.75 0 0 1 .698 0l2.77 1.456-.53-3.084a.75.75 0 0 1 .216-.664l2.24-2.183-3.096-.45a.75.75 0 0 1-.564-.41L8 2.694Z";
const ICON_FORK: &str = "M5 5.372v.878c0 .414.336.75.75.75h4.5a.75.75 0 0 0 .75-.75v-.878a2.25 2.25 0 1 1 1.5 0v.878a2.25 2.25 0 0 1-2.25 2.25h-1.5v2.128a2.251 2.251 0 1 1-1.5 0V8.5h-1.5A2.25 2.25 0 0 1 3.5 6.25v-.878a2.25 2.25 0 1 1 1.5 0ZM5 3.25a.75.75 0 1 0-1.5 0 .75.75 0 0 0 1.5 0Zm6.75.75a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5Zm-3 8.75a.75.75 0 1 0-1.5 0 .75.75 0 0 0 1.5 0Z";
const ICON_PEOPLE: &str = "M2 5.5a3.5 3.5 0 1 1 5.898 2.549 5.508 5.508 0 0 1 3.034 4.084.75.75 0 1 1-1.482.235 4 4 0 0 0-7.9 0 .75.75 0 0 1-1.482-.236A5.507 5.507 0 0 1 3.102 8.05 3.493 3.493 0 0 1 2 5.5ZM11 4a3.001 3.001 0 0 1 2.22 5.018 5.01 5.01 0 0 1 2.56 3.012.749.749 0 0 1-.885.954.752.752 0 0 1-.549-.514 3.507 3.507 0 0 0-2.522-2.372.75.75 0 0 1-.574-.73v-.352a.75.75 0 0 1 .416-.672A1.5 1.5 0 0 0 11 5.5.75.75 0 0 1 11 4Zm-5.5-.5a2 2 0 1 0-.001 3.999A2 2 0 0 0 5.5 3.5Z";
const ICON_PULSE: &str = "M6 2c.306 0 .582.187.696.471L10 10.731l1.304-3.26A.751.751 0 0 1 12 7h3.25a.75.75 0 0 1 0 1.5h-2.742l-1.812 4.528a.751.751 0 0 1-1.392 0L6 4.77 4.696 8.03A.75.75 0 0 1 4 8.5H.75a.75.75 0 0 1 0-1.5h2.742l1.812-4.529A.751.751 0 0 1 6 2Z";
const ICON_CALENDAR: &str = "M4.75 0a.75.75 0 0 1 .75.75V2h5V.75a.75.75 0 0 1 1.5 0V2h1.25c.966 0 1.75.784 1.75 1.75v10.5A1.75 1.75 0 0 1 13.25 16H2.75A1.75 1.75 0 0 1 1 14.25V3.75C1 2.784 1.784 2 2.75 2H4V.75A.75.75 0 0 1 4.75 0ZM2.5 7.5v6.75c0 .138.112.25.25.25h10.5a.25.25 0 0 0 .25-.25V7.5Zm10.75-4H2.75a.25.25 0 0 0-.25.25V6h11V3.75a.25.25 0 0 0-.25-.25Z";
const ICON_CODE: &str = "M4.72 3.22a.75.75 0 0 1 1.06 1.06L2.06 8l3.72 3.72a.75.75 0 1 1-1.06 1.06L.47 8.53a.75.75 0 0 1 0-1.06l4.25-4.25Zm6.56 0a.75.75 0 1 0-1.06 1.06L13.94 8l-3.72 3.72a.75.75 0 1 0 1.06 1.06l4.25-4.25a.75.75 0 0 0 0-1.06l-4.25-4.25Z";
const ICON_COMMIT: &str = "M11.93 8.5a4.002 4.002 0 0 1-7.86 0H.75a.75.75 0 0 1 0-1.5h3.32a4.002 4.002 0 0 1 7.86 0h3.32a.75.75 0 0 1 0 1.5Zm-1.43-.75a2.5 2.5 0 1 0-5 0 2.5 2.5 0 0 0 5 0Z";
const ICON_GRAPH: &str = "M1.5 1.75V13.5h13.75a.75.75 0 0 1 0 1.5H.75a.75.75 0 0 1-.75-.75V1.75a.75.75 0 0 1 1.5 0Zm14.28 2.53-5.25 5.25a.75.75 0 0 1-1.06 0L7 7.06 4.28 9.78a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042l3.25-3.25a.75.75 0 0 1 1.06 0L10 7.94l4.72-4.72a.751.751 0 0 1 1.042.018.751.751 0 0 1 .018 1.042Z";

pub fn escape_xml(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\n' | '\r' | '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

pub fn truncate_chars(input: &str, max_chars: usize) -> String {
    let cleaned: String = input
        .chars()
        .map(|c| match c {
            '\n' | '\r' | '\t' => ' ',
            other => other,
        })
        .collect();
    let trimmed = cleaned.trim();

    if max_chars == 0 {
        return String::new();
    }
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    if max_chars <= 3 {
        return trimmed.chars().take(max_chars).collect();
    }

    let mut out: String = trimmed.chars().take(max_chars - 3).collect();
    let trimmed_end = out.trim_end().len();
    out.truncate(trimmed_end);
    out.push_str("...");
    out
}

pub fn format_disk_usage(kb: u64) -> String {
    const KB_PER_MB: f64 = 1024.0;
    const KB_PER_GB: f64 = 1024.0 * 1024.0;

    if kb as f64 >= KB_PER_GB {
        format!("{:.2} GB", kb as f64 / KB_PER_GB)
    } else if kb >= 1024 {
        format!("{:.1} MB", kb as f64 / KB_PER_MB)
    } else {
        format!("{kb} KB")
    }
}

pub fn format_joined_ago(joined_at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let days = now.signed_duration_since(joined_at).num_days().max(0);
    if days >= 365 {
        let years = days / 365;
        if years == 1 {
            "Joined GitHub 1 year ago".to_string()
        } else {
            format!("Joined GitHub {years} years ago")
        }
    } else if days >= 30 {
        let months = days / 30;
        if months == 1 {
            "Joined GitHub 1 month ago".to_string()
        } else {
            format!("Joined GitHub {months} months ago")
        }
    } else if days == 1 {
        "Joined GitHub 1 day ago".to_string()
    } else {
        format!("Joined GitHub {days} days ago")
    }
}

pub fn format_relative_age(ts: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let delta: Duration = now.signed_duration_since(ts);
    let secs = delta.num_seconds().max(0);

    if secs < 60 {
        "just now".to_string()
    } else if secs < 3_600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3_600)
    } else if secs < 14 * 86_400 {
        format!("{}d ago", secs / 86_400)
    } else if secs < 60 * 86_400 {
        format!("{}w ago", secs / (7 * 86_400))
    } else if secs < 365 * 86_400 {
        format!("{}mo ago", secs / (30 * 86_400))
    } else {
        format!("{}y ago", secs / (365 * 86_400))
    }
}

fn format_tz_label(offset_hours: i32) -> String {
    if offset_hours >= 0 {
        format!("UTC+{offset_hours}")
    } else {
        format!("UTC{offset_hours}")
    }
}

fn pluralize(count: u32, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {plural}")
    }
}

fn sanitize_hex_color(color: &str, fallback: &str) -> String {
    let trimmed = color.trim();
    if (trimmed.len() == 7 || trimmed.len() == 4)
        && trimmed.starts_with('#')
        && trimmed[1..].chars().all(|c| c.is_ascii_hexdigit())
    {
        trimmed.to_string()
    } else {
        fallback.to_string()
    }
}

/// Builds the top-left 2-column stat list dynamically, omitting zero-value optional metrics
/// and keeping both columns evenly balanced regardless of how many rows are active.
pub fn build_stat_rows(data: &MetricsData) -> (Vec<StatRow>, Vec<StatRow>) {
    let mut rows = Vec::with_capacity(11);

    // Always-shown high-signal metrics.
    rows.push(StatRow {
        icon: ICON_CLOCK,
        label: format_joined_ago(data.joined_at, data.generated_at),
    });
    rows.push(StatRow {
        icon: ICON_REPO,
        label: pluralize(data.repo_count, "Repository", "Repositories"),
    });
    rows.push(StatRow {
        icon: ICON_PULSE,
        label: pluralize(
            data.active_repos_90d,
            "Active repo (90d)",
            "Active repos (90d)",
        ),
    });
    rows.push(StatRow {
        icon: ICON_DATABASE,
        label: format!("{} disk used", format_disk_usage(data.total_disk_kb)),
    });
    if let Some(license) = &data.preferred_license {
        let trimmed = license.trim();
        if !trimmed.is_empty() {
            rows.push(StatRow {
                icon: ICON_LAW,
                label: format!("Preferred license: {}", truncate_chars(trimmed, 14)),
            });
        }
    }

    rows.push(StatRow {
        icon: ICON_COMMIT,
        label: pluralize(
            data.contributions_1y,
            "Contribution (1y)",
            "Contributions (1y)",
        ),
    });
    rows.push(StatRow {
        icon: ICON_GRAPH,
        label: pluralize(data.commits_30d, "Commit (30d)", "Commits (30d)"),
    });

    // Conditionally shown only when > 0.
    if data.total_stars > 0 {
        rows.push(StatRow {
            icon: ICON_STAR,
            label: pluralize(data.total_stars, "Star", "Stars"),
        });
    }
    if data.total_forks > 0 {
        rows.push(StatRow {
            icon: ICON_FORK,
            label: pluralize(data.total_forks, "Fork", "Forks"),
        });
    }
    if data.total_releases > 0 {
        rows.push(StatRow {
            icon: ICON_TAG,
            label: pluralize(data.total_releases, "Release", "Releases"),
        });
    }
    if data.followers > 0 {
        rows.push(StatRow {
            icon: ICON_PEOPLE,
            label: pluralize(data.followers, "Follower", "Followers"),
        });
    }

    let split_idx = rows.len().div_ceil(2);
    let right = rows.split_off(split_idx);
    (rows, right)
}

#[derive(Debug, Clone, Copy)]
pub struct IsoGrid {
    pub origin_x: f64,
    pub origin_y: f64,
    pub step_x: f64,
    pub step_y: f64,
    pub tw: f64,
    pub th: f64,
}

/// Computes isometric base center `(cx, cy)` and vertical extrusion height `h` for a calendar cell.
pub fn compute_iso_tile(
    col: usize,
    row: usize,
    count: u32,
    max_count: u32,
    grid: IsoGrid,
) -> (f64, f64, f64) {
    let cx = grid.origin_x + (col as f64 - row as f64) * grid.step_x;
    let cy = grid.origin_y + (col as f64 + row as f64) * grid.step_y;
    let height = if count == 0 || max_count == 0 {
        2.2
    } else {
        let ratio = (f64::from(count) / f64::from(max_count)).clamp(0.0, 1.0);
        3.6 + ratio.sqrt() * 20.4
    };
    (cx, cy, height)
}

#[derive(Debug, Clone, Copy)]
struct RightColumnLayout {
    has_topics: bool,
    iso_origin_y: f64,
    stats_start_y: f64,
    key_y: f64,
    lang_heading_y: f64,
    lang_bar_y: f64,
    lang_legend_ys: [f64; 2],
    repos_heading_y: f64,
    repos_first_y: f64,
    repos_pitch: f64,
    topics_heading_y: f64,
    topics_row_ys: [f64; 2],
}

fn right_column_layout(has_topics: bool) -> RightColumnLayout {
    if has_topics {
        RightColumnLayout {
            has_topics: true,
            iso_origin_y: 60.0,
            stats_start_y: 60.0,
            key_y: 174.0,
            lang_heading_y: 207.0,
            lang_bar_y: 218.0,
            lang_legend_ys: [246.0, 265.0],
            repos_heading_y: 301.0,
            repos_first_y: 326.0,
            repos_pitch: 26.0,
            topics_heading_y: 444.0,
            topics_row_ys: [466.0, 484.0],
        }
    } else {
        RightColumnLayout {
            has_topics: false,
            iso_origin_y: 64.0,
            stats_start_y: 64.0,
            key_y: 182.0,
            lang_heading_y: 224.0,
            lang_bar_y: 236.0,
            lang_legend_ys: [266.0, 287.0],
            repos_heading_y: 332.0,
            repos_first_y: 362.0,
            repos_pitch: 32.0,
            topics_heading_y: 0.0,
            topics_row_ys: [0.0, 0.0],
        }
    }
}

pub fn render_metrics_svg(data: &MetricsData, theme: Theme) -> String {
    let palette = theme.palette();
    let right_layout = right_column_layout(!data.topics.is_empty());
    let mut svg = String::with_capacity(28_000);

    let login_esc = escape_xml(&data.login);
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {WIDTH} {HEIGHT}" width="{WIDTH}" height="{HEIGHT}" role="img" aria-label="{login_esc} GitHub metrics">"#
    );

    let _ = writeln!(
        svg,
        r#"  <defs>
    <clipPath id="lang-bar-clip">
      <rect x="472" y="{:.0}" width="432" height="8" rx="4"/>
    </clipPath>
  </defs>
  <style>
    .sans {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif; }}
    .mono {{ font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }}
  </style>"#,
        right_layout.lang_bar_y
    );

    render_left_column(&mut svg, data, palette);
    render_right_column(&mut svg, data, palette, right_layout);

    svg.push_str("</svg>\n");
    svg
}

fn render_icon(svg: &mut String, x: f64, y: f64, size: f64, path: &str, color: &str) {
    let scale = size / 16.0;
    let _ = writeln!(
        svg,
        r#"    <g transform="translate({x:.1},{y:.1}) scale({scale:.4})"><path d="{path}" fill="{color}"/></g>"#
    );
}

fn render_section_heading(
    svg: &mut String,
    x: f64,
    y: f64,
    icon_path: &str,
    title: &str,
    palette: Palette,
) {
    render_icon(svg, x, y - 13.0, 15.0, icon_path, palette.accent_header);
    let title_esc = escape_xml(title);
    let _ = writeln!(
        svg,
        r#"    <text x="{:.1}" y="{y:.1}" class="sans" font-size="15" font-weight="600" fill="{}">{title_esc}</text>"#,
        x + 21.0,
        palette.accent_header
    );
}

fn render_left_column(svg: &mut String, data: &MetricsData, palette: Palette) {
    svg.push_str("  <!-- Left Column -->\n");
    svg.push_str("  <g>\n");

    // Header: username + 12-square mini activity bar.
    render_icon(svg, 16.0, 14.5, 18.0, ICON_PERSON, palette.accent_header);
    let login_esc = escape_xml(&truncate_chars(&data.login, 24));
    let _ = writeln!(
        svg,
        r#"    <text x="40" y="30" class="sans" font-size="19" font-weight="600" fill="{}">{login_esc}</text>"#,
        palette.accent_header
    );

    let sq_size = 9.0;
    let sq_gap = 3.0;
    let bar_start_x = 448.0 - (12.0 * sq_size + 11.0 * sq_gap);
    for idx in 0..12 {
        let level = data
            .recent_12_levels
            .get(idx)
            .copied()
            .unwrap_or(0)
            .min(4) as usize;
        let fill = palette.iso_levels[level].0;
        let sx = bar_start_x + idx as f64 * (sq_size + sq_gap);
        let _ = writeln!(
            svg,
            r#"    <rect x="{sx:.1}" y="19" width="{sq_size:.0}" height="{sq_size:.0}" rx="2" fill="{fill}"/>"#
        );
    }

    // Dynamic 2-column compact icon stat list with balanced vertical spacing.
    let (left_stats, right_stats) = build_stat_rows(data);
    let max_rows = left_stats.len().max(right_stats.len());
    let (start_y, row_pitch) = match max_rows {
        0..=3 => (64.0, 24.0),
        4 => (62.0, 22.0),
        5 => (60.0, 20.5),
        _ => (57.0, 19.0),
    };

    for (idx, row) in left_stats.iter().enumerate() {
        let row_y = start_y + idx as f64 * row_pitch;
        render_icon(svg, 16.0, row_y - 11.5, 13.5, row.icon, palette.text_muted);
        let text_esc = escape_xml(&truncate_chars(&row.label, 31));
        let _ = writeln!(
            svg,
            r#"    <text x="36" y="{row_y:.1}" class="sans" font-size="12" fill="{}">{text_esc}</text>"#,
            palette.text_primary
        );
    }

    for (idx, row) in right_stats.iter().enumerate() {
        let row_y = start_y + idx as f64 * row_pitch;
        render_icon(svg, 250.0, row_y - 11.5, 13.5, row.icon, palette.text_muted);
        let text_esc = escape_xml(&truncate_chars(&row.label, 27));
        let _ = writeln!(
            svg,
            r#"    <text x="270" y="{row_y:.1}" class="sans" font-size="12" fill="{}">{text_esc}</text>"#,
            palette.text_primary
        );
    }

    let last_stat_y = if max_rows == 0 {
        start_y
    } else {
        start_y + (max_rows - 1) as f64 * row_pitch
    };
    let habits_heading_y = (last_stat_y + 34.0).clamp(154.0, 184.0);

    // Coding habits and recent activity.
    render_section_heading(
        svg,
        16.0,
        habits_heading_y,
        ICON_PULSE,
        "Coding habits and recent activity",
        palette,
    );
    let tz_label = format_tz_label(data.tz_offset_hours);
    let _ = writeln!(
        svg,
        r#"    <text x="448" y="{habits_heading_y:.1}" text-anchor="end" class="sans" font-size="10.5" fill="{}">{tz_label}</text>"#,
        palette.text_muted
    );

    render_hourly_chart(svg, &data.hourly_commits, habits_heading_y + 22.0, palette);
    render_weekday_chart(svg, &data.weekday_commits, palette);
    render_recent_languages(svg, &data.recent_languages, palette);

    svg.push_str("  </g>\n");
}

fn render_hourly_chart(
    svg: &mut String,
    hourly: &[u32; 24],
    subtitle_y: f64,
    palette: Palette,
) {
    let _ = writeln!(
        svg,
        r#"    <text x="16" y="{subtitle_y:.1}" class="sans" font-size="11.5" fill="{}">Commit activity per time of the day</text>"#,
        palette.text_muted
    );

    let plot_x = 18.0;
    let plot_w = 428.0;
    let baseline_y = 304.0;
    let top_limit_y = subtitle_y + 24.0;
    let max_bar_h = (baseline_y - top_limit_y).clamp(62.0, 92.0);
    let slot_w = plot_w / 24.0;
    let bar_w = 10.0;
    let max_val = hourly.iter().copied().max().unwrap_or(0);

    let _ = writeln!(
        svg,
        r#"    <line x1="16" y1="{:.1}" x2="448" y2="{:.1}" stroke="{}" stroke-width="1" stroke-opacity="0.6"/>"#,
        baseline_y + 1.0,
        baseline_y + 1.0,
        palette.axis_line
    );

    for (hour, &count) in hourly.iter().enumerate() {
        let center_x = plot_x + (hour as f64 + 0.5) * slot_w;
        let bar_x = center_x - bar_w * 0.5;

        if count > 0 && max_val > 0 {
            let bar_h = ((f64::from(count) / f64::from(max_val)) * max_bar_h).max(3.0);
            let bar_y = baseline_y - bar_h;
            let _ = writeln!(
                svg,
                r#"    <rect x="{bar_x:.1}" y="{bar_y:.1}" width="{bar_w:.1}" height="{bar_h:.1}" rx="2" fill="{}"/>"#,
                palette.bar_fill
            );
            let _ = writeln!(
                svg,
                r#"    <text x="{center_x:.1}" y="{:.1}" text-anchor="middle" class="mono" font-size="8" fill="{}">{count}</text>"#,
                bar_y - 4.0,
                palette.text_muted
            );
        } else {
            let _ = writeln!(
                svg,
                r#"    <rect x="{bar_x:.1}" y="{:.1}" width="{bar_w:.1}" height="2" rx="1" fill="{}"/>"#,
                baseline_y - 2.0,
                palette.track_fill
            );
        }

        let _ = writeln!(
            svg,
            r#"    <text x="{center_x:.1}" y="319" text-anchor="middle" class="mono" font-size="8.5" fill="{}">{hour:02}</text>"#,
            palette.text_muted
        );
    }
}

fn render_weekday_chart(svg: &mut String, weekday: &[u32; 7], palette: Palette) {
    let _ = writeln!(
        svg,
        r#"    <text x="16" y="348" class="sans" font-size="11.5" fill="{}">Commit activity per day</text>"#,
        palette.text_muted
    );

    let labels = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let plot_x = 18.0;
    let plot_w = 200.0;
    let baseline_y = 462.0;
    let max_bar_h = 84.0;
    let slot_w = plot_w / 7.0;
    let bar_w = 14.0;
    let max_val = weekday.iter().copied().max().unwrap_or(0);

    let _ = writeln!(
        svg,
        r#"    <line x1="16" y1="{:.1}" x2="220" y2="{:.1}" stroke="{}" stroke-width="1" stroke-opacity="0.6"/>"#,
        baseline_y + 1.0,
        baseline_y + 1.0,
        palette.axis_line
    );

    for (day_idx, &count) in weekday.iter().enumerate() {
        let center_x = plot_x + (day_idx as f64 + 0.5) * slot_w;
        let bar_x = center_x - bar_w * 0.5;

        if count > 0 && max_val > 0 {
            let bar_h = ((f64::from(count) / f64::from(max_val)) * max_bar_h).max(3.0);
            let bar_y = baseline_y - bar_h;
            let _ = writeln!(
                svg,
                r#"    <rect x="{bar_x:.1}" y="{bar_y:.1}" width="{bar_w:.1}" height="{bar_h:.1}" rx="2.5" fill="{}"/>"#,
                palette.bar_fill
            );
            let _ = writeln!(
                svg,
                r#"    <text x="{center_x:.1}" y="{:.1}" text-anchor="middle" class="mono" font-size="8.5" fill="{}">{count}</text>"#,
                bar_y - 4.0,
                palette.text_muted
            );
        } else {
            let _ = writeln!(
                svg,
                r#"    <rect x="{bar_x:.1}" y="{:.1}" width="{bar_w:.1}" height="2" rx="1" fill="{}"/>"#,
                baseline_y - 2.0,
                palette.track_fill
            );
        }

        let _ = writeln!(
            svg,
            r#"    <text x="{center_x:.1}" y="477" text-anchor="middle" class="mono" font-size="8.5" fill="{}">{}</text>"#,
            palette.text_muted,
            labels[day_idx]
        );
    }
}

fn render_recent_languages(svg: &mut String, languages: &[LanguageShare], palette: Palette) {
    let _ = writeln!(
        svg,
        r#"    <text x="242" y="348" class="sans" font-size="11.5" fill="{}">Recent language activity</text>"#,
        palette.text_muted
    );

    if languages.is_empty() {
        let _ = writeln!(
            svg,
            r#"    <text x="242" y="376" class="sans" font-size="11.5" fill="{}">No recent repository pushes (90d)</text>"#,
            palette.text_muted
        );
        return;
    }

    let bar_x = 242.0;
    let bar_w = 206.0;
    let bar_h = 6.0;

    for (idx, lang) in languages.iter().take(4).enumerate() {
        let row_y = 374.0 + idx as f64 * 29.0;
        let name_esc = escape_xml(&truncate_chars(&lang.name, 16));
        let pct_str = format!("{:.1}%", lang.percent);
        let fill_color = sanitize_hex_color(&lang.color, palette.bar_fill);
        let fill_w = ((lang.percent / 100.0) * bar_w).clamp(3.0, bar_w);

        let _ = writeln!(
            svg,
            r#"    <text x="{bar_x:.0}" y="{row_y:.1}" class="sans" font-size="11.5" fill="{}">{name_esc}</text>"#,
            palette.text_primary
        );
        let _ = writeln!(
            svg,
            r#"    <text x="448" y="{row_y:.1}" text-anchor="end" class="sans" font-size="11" fill="{}">{pct_str}</text>"#,
            palette.text_muted
        );
        let track_y = row_y + 6.0;
        let _ = writeln!(
            svg,
            r#"    <rect x="{bar_x:.0}" y="{track_y:.1}" width="{bar_w:.0}" height="{bar_h:.0}" rx="3" fill="{}"/>"#,
            palette.track_fill
        );
        let _ = writeln!(
            svg,
            r#"    <rect x="{bar_x:.0}" y="{track_y:.1}" width="{fill_w:.1}" height="{bar_h:.0}" rx="3" fill="{fill_color}"/>"#
        );
    }
}

fn render_right_column(
    svg: &mut String,
    data: &MetricsData,
    palette: Palette,
    layout: RightColumnLayout,
) {
    svg.push_str("  <!-- Right Column -->\n");
    svg.push_str("  <g>\n");

    render_isometric_calendar(svg, data, palette, layout);
    render_most_used_languages(svg, &data.languages, palette, layout);
    render_active_repositories(svg, &data.active_repos, data.generated_at, palette, layout);
    if layout.has_topics {
        render_repository_topics(svg, &data.topics, palette, layout);
    }

    svg.push_str("  </g>\n");
}

fn render_isometric_calendar(
    svg: &mut String,
    data: &MetricsData,
    palette: Palette,
    layout: RightColumnLayout,
) {
    render_section_heading(
        svg,
        472.0,
        30.0,
        ICON_CALENDAR,
        "Contributions calendar",
        palette,
    );
    let _ = writeln!(
        svg,
        r#"    <text x="904" y="30" text-anchor="end" class="sans" font-size="10.5" fill="{}">Last 6 months</text>"#,
        palette.text_muted
    );

    let max_count = data
        .calendar_weeks
        .iter()
        .flatten()
        .map(|d| d.count)
        .max()
        .unwrap_or(0);

    let grid = IsoGrid {
        origin_x: 532.0,
        origin_y: layout.iso_origin_y,
        step_x: 8.4,
        step_y: 3.55,
        tw: 7.5,
        th: 3.15,
    };

    svg.push_str("    <g>\n");
    for (col, week) in data.calendar_weeks.iter().enumerate() {
        for (row, day) in week.iter().enumerate() {
            render_iso_prism(svg, col, row, day, max_count, grid, palette);
        }
    }
    svg.push_str("    </g>\n");

    let stats_x = 904.0;
    let s0 = layout.stats_start_y;
    let _ = writeln!(
        svg,
        r#"    <text x="{stats_x:.0}" y="{s0:.0}" text-anchor="end" class="sans" font-size="10.5" fill="{}">Current streak</text>"#,
        palette.text_muted
    );
    let current_unit = if data.current_streak == 1 { "day" } else { "days" };
    let _ = writeln!(
        svg,
        r#"    <text x="{stats_x:.0}" y="{:.0}" text-anchor="end" class="sans" font-size="14" font-weight="600" fill="{}">{} {current_unit}</text>"#,
        s0 + 17.0,
        palette.text_primary,
        data.current_streak
    );

    let _ = writeln!(
        svg,
        r#"    <text x="{stats_x:.0}" y="{:.0}" text-anchor="end" class="sans" font-size="10.5" fill="{}">Longest streak</text>"#,
        s0 + 42.0,
        palette.text_muted
    );
    let longest_unit = if data.longest_streak == 1 { "day" } else { "days" };
    let _ = writeln!(
        svg,
        r#"    <text x="{stats_x:.0}" y="{:.0}" text-anchor="end" class="sans" font-size="14" font-weight="600" fill="{}">{} {longest_unit}</text>"#,
        s0 + 59.0,
        palette.text_primary,
        data.longest_streak
    );

    let _ = writeln!(
        svg,
        r#"    <text x="{stats_x:.0}" y="{:.0}" text-anchor="end" class="sans" font-size="10.5" fill="{}">Daily average</text>"#,
        s0 + 84.0,
        palette.text_muted
    );
    let _ = writeln!(
        svg,
        r#"    <text x="{stats_x:.0}" y="{:.0}" text-anchor="end" class="sans" font-size="14" font-weight="600" fill="{}">{:.1} / day</text>"#,
        s0 + 101.0,
        palette.text_primary,
        data.avg_commits_per_day
    );

    let key_y = layout.key_y;
    let _ = writeln!(
        svg,
        r#"    <text x="815" y="{:.1}" text-anchor="end" class="sans" font-size="9.5" fill="{}">Less</text>"#,
        key_y + 7.5,
        palette.text_muted
    );
    for (level, &(fill, _, _)) in palette.iso_levels.iter().enumerate() {
        let kx = 821.0 + level as f64 * 11.0;
        let _ = writeln!(
            svg,
            r#"    <rect x="{kx:.0}" y="{key_y:.0}" width="8" height="8" rx="1.5" fill="{fill}"/>"#
        );
    }
    let _ = writeln!(
        svg,
        r#"    <text x="904" y="{:.1}" text-anchor="end" class="sans" font-size="9.5" fill="{}">More</text>"#,
        key_y + 7.5,
        palette.text_muted
    );
}

fn render_iso_prism(
    svg: &mut String,
    col: usize,
    row: usize,
    day: &ContributionDay,
    max_count: u32,
    grid: IsoGrid,
    palette: Palette,
) {
    let (cx, cy, h) = compute_iso_tile(col, row, day.count, max_count, grid);
    let (top_col, left_col, right_col) = palette.iso_levels[usize::from(day.level.min(4))];
    let tw = grid.tw;
    let th = grid.th;

    let top_pts = format!(
        "{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
        cx,
        cy - th - h,
        cx + tw,
        cy - h,
        cx,
        cy + th - h,
        cx - tw,
        cy - h,
    );
    let left_pts = format!(
        "{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
        cx - tw,
        cy - h,
        cx,
        cy + th - h,
        cx,
        cy + th,
        cx - tw,
        cy,
    );
    let right_pts = format!(
        "{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
        cx + tw,
        cy - h,
        cx,
        cy + th - h,
        cx,
        cy + th,
        cx + tw,
        cy,
    );

    let _ = writeln!(
        svg,
        r#"      <polygon points="{left_pts}" fill="{left_col}"/>"#
    );
    let _ = writeln!(
        svg,
        r#"      <polygon points="{right_pts}" fill="{right_col}"/>"#
    );
    let _ = writeln!(
        svg,
        r#"      <polygon points="{top_pts}" fill="{top_col}"/>"#
    );
}

fn estimate_sans_width(text: &str, font_size: f64) -> f64 {
    let scale = font_size / 11.0;
    let base: f64 = text
        .chars()
        .map(|c| match c {
            'i' | 'l' | 'j' | 'I' | '!' | '.' | ',' | ':' | ';' | '|' => 3.3,
            'f' | 'r' | 't' | '-' | ' ' => 4.1,
            'm' | 'w' | 'M' | 'W' => 8.6,
            'A'..='Z' => 7.0,
            '0'..='9' => 6.1,
            '#' | '%' | '@' => 7.8,
            _ => 5.9,
        })
        .sum();
    base * scale
}

fn render_most_used_languages(
    svg: &mut String,
    languages: &[LanguageShare],
    palette: Palette,
    layout: RightColumnLayout,
) {
    render_section_heading(
        svg,
        472.0,
        layout.lang_heading_y,
        ICON_CODE,
        "Most used languages",
        palette,
    );

    let bar_x: f64 = 472.0;
    let bar_y: f64 = layout.lang_bar_y;
    let bar_w: f64 = 432.0;
    let bar_h: f64 = 8.0;

    let _ = writeln!(
        svg,
        r#"    <rect x="{bar_x:.0}" y="{bar_y:.0}" width="{bar_w:.0}" height="{bar_h:.0}" rx="4" fill="{}"/>"#,
        palette.track_fill
    );

    if languages.is_empty() {
        let _ = writeln!(
            svg,
            r#"    <text x="472" y="{:.1}" class="sans" font-size="11.5" fill="{}">No language data available</text>"#,
            layout.lang_legend_ys[0], palette.text_muted
        );
        return;
    }

    let total_weight: f64 = languages.iter().map(|l| l.weight).sum();
    if total_weight > 0.0 {
        svg.push_str(r#"    <g clip-path="url(#lang-bar-clip)">"#);
        svg.push('\n');
        let mut cursor_x = bar_x;
        for (idx, lang) in languages.iter().enumerate() {
            let color = sanitize_hex_color(&lang.color, palette.text_muted);
            let seg_w = if idx + 1 == languages.len() {
                (bar_x + bar_w - cursor_x).max(0.0)
            } else {
                ((lang.weight / total_weight) * bar_w).max(2.0)
            };
            let _ = writeln!(
                svg,
                r#"      <rect x="{cursor_x:.2}" y="{bar_y:.0}" width="{seg_w:.2}" height="{bar_h:.0}" fill="{color}"/>"#
            );
            cursor_x += seg_w;
        }
        svg.push_str("    </g>\n");
    }

    let mut row = 0usize;
    let mut cursor_x = 472.0;
    let max_x = 904.0;
    let row_ys = layout.lang_legend_ys;

    for lang in languages {
        let name = truncate_chars(&lang.name, 13);
        let pct = format!("{:.1}%", lang.percent);
        let est_w = 11.0 + estimate_sans_width(&name, 11.0) + 4.0 + estimate_sans_width(&pct, 11.0);

        if cursor_x + est_w > max_x && cursor_x > 472.0 {
            row += 1;
            if row >= row_ys.len() {
                break;
            }
            cursor_x = 472.0;
        }

        let y = row_ys[row];
        let color = sanitize_hex_color(&lang.color, palette.text_muted);
        let name_esc = escape_xml(&name);

        let _ = writeln!(
            svg,
            r#"    <circle cx="{:.1}" cy="{:.1}" r="3.5" fill="{color}"/>"#,
            cursor_x + 3.5,
            y - 3.5
        );
        let _ = writeln!(
            svg,
            r#"    <text x="{:.1}" y="{y:.1}" class="sans" font-size="11"><tspan fill="{}" font-weight="500">{name_esc}</tspan><tspan fill="{}"> {pct}</tspan></text>"#,
            cursor_x + 11.0,
            palette.text_primary,
            palette.text_muted
        );

        cursor_x += est_w + 13.0;
    }
}

fn render_active_repositories(
    svg: &mut String,
    repos: &[ActiveRepo],
    now: DateTime<Utc>,
    palette: Palette,
    layout: RightColumnLayout,
) {
    render_section_heading(
        svg,
        472.0,
        layout.repos_heading_y,
        ICON_REPO,
        "Most active repositories",
        palette,
    );

    if repos.is_empty() {
        let _ = writeln!(
            svg,
            r#"    <text x="472" y="{:.1}" class="sans" font-size="11.5" fill="{}">No public repositories found</text>"#,
            layout.repos_first_y, palette.text_muted
        );
        return;
    }

    for (idx, repo) in repos.iter().take(4).enumerate() {
        let row_y = layout.repos_first_y + idx as f64 * layout.repos_pitch;
        render_icon(svg, 472.0, row_y - 11.0, 13.0, ICON_REPO, palette.text_muted);

        let name_esc = escape_xml(&truncate_chars(&repo.name, 22));
        let _ = writeln!(
            svg,
            r#"    <text x="490" y="{row_y:.1}" class="sans" font-size="12.5" font-weight="600" fill="{}">{name_esc}</text>"#,
            palette.text_primary
        );

        if let Some(lang) = &repo.primary_language {
            let dot_color = repo
                .primary_language_color
                .as_deref()
                .map(|c| sanitize_hex_color(c, palette.text_muted))
                .unwrap_or_else(|| palette.text_muted.to_string());
            let lang_esc = escape_xml(&truncate_chars(lang, 12));
            let _ = writeln!(
                svg,
                r#"    <circle cx="696" cy="{:.1}" r="3.5" fill="{dot_color}"/>"#,
                row_y - 3.8
            );
            let _ = writeln!(
                svg,
                r#"    <text x="704" y="{row_y:.1}" class="sans" font-size="11" fill="{}">{lang_esc}</text>"#,
                palette.text_muted
            );
        }

        render_icon(svg, 796.0, row_y - 10.5, 12.0, ICON_STAR, palette.text_muted);
        let _ = writeln!(
            svg,
            r#"    <text x="812" y="{row_y:.1}" class="sans" font-size="11" fill="{}">{}</text>"#,
            palette.text_muted, repo.stars
        );

        let age = escape_xml(&format_relative_age(repo.pushed_at, now));
        let _ = writeln!(
            svg,
            r#"    <text x="904" y="{row_y:.1}" text-anchor="end" class="sans" font-size="11" fill="{}">{age}</text>"#,
            palette.text_muted
        );
    }
}

fn render_repository_topics(
    svg: &mut String,
    topics: &[String],
    palette: Palette,
    layout: RightColumnLayout,
) {
    render_section_heading(
        svg,
        472.0,
        layout.topics_heading_y,
        ICON_TAG,
        "Repository topics",
        palette,
    );

    let mut row = 0usize;
    let mut cursor_x = 472.0;
    let max_x = 904.0;
    let row_ys = layout.topics_row_ys;

    for topic in topics {
        let clean = truncate_chars(topic, 18);
        let est_w = estimate_sans_width("#", 11.5) + estimate_sans_width(&clean, 11.5);

        if cursor_x + est_w > max_x && cursor_x > 472.0 {
            row += 1;
            if row >= row_ys.len() {
                break;
            }
            cursor_x = 472.0;
        }

        let y = row_ys[row];
        let topic_esc = escape_xml(&clean);
        let _ = writeln!(
            svg,
            r#"    <text x="{cursor_x:.1}" y="{y:.1}" class="sans" font-size="11.5"><tspan fill="{}">#</tspan><tspan fill="{}">{topic_esc}</tspan></text>"#,
            palette.accent_header, palette.text_muted
        );

        cursor_x += est_w + 13.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{bucket_commit_timestamps, compute_streaks, mock_metrics};
    use chrono::{NaiveDate, TimeZone};

    #[test]
    fn xml_escaping_escapes_reserved_chars_and_control_bytes() {
        assert_eq!(
            escape_xml(r#"<repo> & "name" 'ok'\n\t"#),
            "&lt;repo&gt; &amp; &quot;name&quot; &apos;ok&apos;\\n\\t"
        );
        assert_eq!(escape_xml("a\nb\tc\x00d"), "a b cd");
    }

    #[test]
    fn utf8_truncation_is_safe_on_multibyte_boundaries() {
        assert_eq!(truncate_chars("日本語リポジトリ", 6), "日本語...");
        assert_eq!(truncate_chars("café", 4), "café");
        assert_eq!(truncate_chars("café", 2), "ca");
        assert_eq!(truncate_chars("hello", 0), "");
    }

    #[test]
    fn disk_usage_formats_kb_mb_gb_cleanly() {
        assert_eq!(format_disk_usage(512), "512 KB");
        assert_eq!(format_disk_usage(10_240), "10.0 MB");
        assert_eq!(format_disk_usage(2_097_152), "2.00 GB");
    }

    #[test]
    fn dynamic_stat_rows_omit_zero_values_and_balance_columns() {
        let mut data = mock_metrics("jpmasangkay", 8);
        data.followers = 0;
        data.total_stars = 0;
        data.total_forks = 0;
        data.total_releases = 0;
        data.preferred_license = None;

        let (left, right) = build_stat_rows(&data);
        assert_eq!(left.len(), 3);
        assert_eq!(right.len(), 3);

        let all_labels: Vec<&str> = left
            .iter()
            .chain(right.iter())
            .map(|r| r.label.as_str())
            .collect();
        assert!(all_labels.iter().any(|l| l.starts_with("Joined GitHub")));
        assert!(all_labels.iter().any(|l| l.contains("Repositories")));
        assert!(all_labels.iter().any(|l| l.contains("Contributions (1y)")));
        assert!(all_labels.iter().any(|l| l.contains("Commits (30d)")));
        assert!(all_labels.iter().any(|l| l.contains("Active repos (90d)")));
        assert!(all_labels.iter().any(|l| l.contains("disk used")));

        for forbidden in ["Follower", "Star", "Fork", "Release", "Preferred license"] {
            assert!(
                !all_labels.iter().any(|l| l.contains(forbidden)),
                "unexpected zero/empty stat row containing '{forbidden}'"
            );
        }

        // Enable two optional metrics -> 8 rows split evenly 4/4.
        data.total_stars = 5;
        data.preferred_license = Some("MIT".to_string());
        let (left2, right2) = build_stat_rows(&data);
        assert_eq!(left2.len(), 4);
        assert_eq!(right2.len(), 4);
        let combined: Vec<&str> = left2
            .iter()
            .chain(right2.iter())
            .map(|r| r.label.as_str())
            .collect();
        assert!(combined.contains(&"5 Stars"));
        assert!(combined.contains(&"Preferred license: MIT"));
        assert!(!combined.iter().any(|l| l.contains("Fork") || l.contains("Release") || l.contains("Follower")));
    }

    #[test]
    fn empty_topics_omits_section_cleanly() {
        let mut data = mock_metrics("jpmasangkay", 8);
        data.topics.clear();

        let svg = render_metrics_svg(&data, Theme::Dark);
        assert!(!svg.contains("Repository topics"));
        assert!(!svg.contains("No repository topics set"));
        // ClipPath y must match tightened language bar y (236).
        assert!(svg.contains(r#"<rect x="472" y="236" width="432" height="8" rx="4"/>"#));

        data.topics = vec!["rust".to_string()];
        let svg_with_topics = render_metrics_svg(&data, Theme::Dark);
        assert!(svg_with_topics.contains("Repository topics"));
        assert!(svg_with_topics.contains(">rust<"));
    }

    #[test]
    fn isometric_tile_math_handles_zero_and_peak_days() {
        let grid = IsoGrid {
            origin_x: 500.0,
            origin_y: 60.0,
            step_x: 8.4,
            step_y: 3.55,
            tw: 7.5,
            th: 3.15,
        };
        let (cx0, cy0, h0) = compute_iso_tile(0, 0, 0, 0, grid);
        assert!((cx0 - 500.0).abs() < 1e-6);
        assert!((cy0 - 60.0).abs() < 1e-6);
        assert!(h0.is_finite() && h0 > 0.0);

        let (_, _, h_mid) = compute_iso_tile(5, 3, 5, 20, grid);
        let (_, _, h_peak) = compute_iso_tile(5, 3, 20, 20, grid);
        assert!(h_peak > h_mid);
        assert!(h_mid > h0);
    }

    #[test]
    fn timezone_bucketing_applies_utc_offset() {
        let ts = Utc.with_ymd_and_hms(2026, 6, 15, 16, 30, 0).unwrap();
        let (hours, days) = bucket_commit_timestamps(&[ts], 8);
        assert_eq!(hours[0], 1);
        assert_eq!(hours[16], 0);
        assert_eq!(days[2], 1);
    }

    #[test]
    fn streak_calculation_handles_active_and_broken_runs() {
        let base = NaiveDate::from_ymd_opt(2026, 11, 10).unwrap();
        let counts = [2, 3, 4, 0, 1, 2, 0];
        let days: Vec<ContributionDay> = counts
            .iter()
            .enumerate()
            .map(|(i, &c)| ContributionDay {
                date: base + Duration::days(i as i64),
                count: c,
                level: if c > 0 { 2 } else { 0 },
            })
            .collect();
        let refs: Vec<&ContributionDay> = days.iter().collect();

        let today = base + Duration::days(6);
        let (current, longest) = compute_streaks(&refs, today);
        assert_eq!(current, 2);
        assert_eq!(longest, 3);
    }

    #[test]
    fn rendered_dark_and_light_svgs_are_transparent_borderless_and_self_contained() {
        let data = mock_metrics("jpmasangkay", 8);
        for theme in [Theme::Dark, Theme::Light] {
            let rendered = render_metrics_svg(&data, theme);
            assert!(rendered.contains(r#"viewBox="0 0 920 500""#));
            assert!(!rendered.contains("scanlines"));
            assert!(!rendered.contains("OPEN TO"));
            assert!(!rendered.contains("<script"));
            assert!(!rendered.contains("@import"));
            assert!(!rendered.contains("filter="));
            let without_xmlns = rendered.replace(r#"xmlns="http://www.w3.org/2000/svg""#, "");
            assert!(!without_xmlns.contains("http://"));
            assert!(!without_xmlns.contains("https://"));
        }
    }
}
