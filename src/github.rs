use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Datelike, Duration as ChronoDuration, FixedOffset, NaiveDate, Timelike, Utc};
use serde::Deserialize;
use serde_json::json;

const GITHUB_GRAPHQL_ENDPOINT: &str = "https://api.github.com/graphql";
const USER_AGENT: &str = "jpmasangkay-profile-metrics";
const MAX_HABIT_COMMITS: usize = 200;
const CALENDAR_WEEKS: usize = 26;
const RECENT_COMMITS_DAYS: i64 = 30;
const RECENT_ACTIVITY_DAYS: i64 = 90;
const TOP_LANGUAGES_LIMIT: usize = 8;
const RECENT_LANGUAGES_LIMIT: usize = 4;
const TOP_ACTIVE_REPOS_LIMIT: usize = 4;
const TOP_TOPICS_LIMIT: usize = 10;
const TOP_DIAGNOSTIC_REPOS_LIMIT: usize = 5;

const OVERVIEW_QUERY: &str = r#"
query UserOverview($login: String!, $from: DateTime!, $to: DateTime!) {
  user(login: $login) {
    login
    id
    createdAt
    followers {
      totalCount
    }
    contributionsCollection(from: $from, to: $to) {
      contributionCalendar {
        totalContributions
        weeks {
          contributionDays {
            date
            contributionCount
            contributionLevel
          }
        }
      }
    }
  }
}
"#;

const REPOSITORIES_QUERY: &str = r#"
query OwnedRepositories($login: String!, $cursor: String, $authorId: ID!) {
  user(login: $login) {
    repositories(
      first: 50
      after: $cursor
      ownerAffiliations: OWNER
      isFork: false
      orderBy: { field: PUSHED_AT, direction: DESC }
    ) {
      pageInfo {
        hasNextPage
        endCursor
      }
      nodes {
        name
        isPrivate
        stargazerCount
        forkCount
        diskUsage
        pushedAt
        isArchived
        licenseInfo {
          spdxId
          name
        }
        releases {
          totalCount
        }
        primaryLanguage {
          name
          color
        }
        languages(first: 10, orderBy: { field: SIZE, direction: DESC }) {
          edges {
            size
            node {
              name
              color
            }
          }
        }
        repositoryTopics(first: 10) {
          nodes {
            topic {
              name
            }
          }
        }
        defaultBranchRef {
          target {
            ... on Commit {
              history(first: 20, author: { id: $authorId }) {
                nodes {
                  oid
                  committedDate
                }
              }
            }
          }
        }
      }
    }
  }
}
"#;

#[derive(Debug, Clone)]
pub struct ContributionDay {
    pub date: NaiveDate,
    pub count: u32,
    pub level: u8,
}

#[derive(Debug, Clone)]
pub struct LanguageShare {
    pub name: String,
    pub color: String,
    pub weight: f64,
    pub percent: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveRepo {
    pub name: String,
    pub primary_language: Option<String>,
    pub primary_language_color: Option<String>,
    pub stars: u32,
    pub pushed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSizeDiagnostic {
    pub name: String,
    pub raw_bytes: u64,
    pub primary_language: String,
}

#[derive(Debug, Clone)]
pub struct RepoLanguageSample {
    pub name: String,
    pub bytes: u64,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct RepoSummary {
    pub name: String,
    pub is_private: bool,
    pub stars: u32,
    pub pushed_at: Option<DateTime<Utc>>,
    pub primary_language: Option<String>,
    pub primary_language_color: Option<String>,
    pub languages: Vec<RepoLanguageSample>,
    pub topics: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MetricsData {
    pub login: String,
    pub joined_at: DateTime<Utc>,
    pub followers: u32,
    pub repo_count: u32,
    pub active_repos_90d: u32,
    pub total_stars: u32,
    pub total_forks: u32,
    pub total_releases: u32,
    pub total_disk_kb: u64,
    pub preferred_license: Option<String>,
    pub contributions_1y: u32,
    pub commits_30d: u32,
    pub tz_offset_hours: i32,
    pub hourly_commits: [u32; 24],
    pub weekday_commits: [u32; 7],
    pub calendar_weeks: Vec<Vec<ContributionDay>>,
    pub recent_12_levels: Vec<u8>,
    pub current_streak: u32,
    pub longest_streak: u32,
    pub avg_commits_per_day: f64,
    pub languages: Vec<LanguageShare>,
    pub recent_languages: Vec<LanguageShare>,
    pub active_repos: Vec<ActiveRepo>,
    pub topics: Vec<String>,
    pub top_public_repos_by_bytes: Vec<RepoSizeDiagnostic>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct GraphQlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQlError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct OverviewData {
    user: Option<OverviewUser>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OverviewUser {
    login: String,
    id: String,
    created_at: String,
    followers: CountNode,
    contributions_collection: ContributionsCollection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CountNode {
    total_count: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContributionsCollection {
    contribution_calendar: ContributionCalendarNode,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContributionCalendarNode {
    total_contributions: u32,
    weeks: Vec<CalendarWeekNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CalendarWeekNode {
    contribution_days: Vec<CalendarDayNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CalendarDayNode {
    date: String,
    contribution_count: u32,
    contribution_level: String,
}

#[derive(Debug, Deserialize)]
struct ReposData {
    user: Option<ReposUser>,
}

#[derive(Debug, Deserialize)]
struct ReposUser {
    repositories: RepoConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepoConnection {
    page_info: PageInfo,
    nodes: Vec<Option<RepoNode>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepoNode {
    name: String,
    is_private: bool,
    stargazer_count: u32,
    fork_count: u32,
    disk_usage: Option<u64>,
    pushed_at: Option<String>,
    #[allow(dead_code)]
    is_archived: bool,
    license_info: Option<LicenseNode>,
    releases: CountNode,
    primary_language: Option<LanguageNode>,
    languages: LanguageConnection,
    repository_topics: TopicConnection,
    default_branch_ref: Option<BranchRef>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LicenseNode {
    spdx_id: Option<String>,
    name: String,
}

#[derive(Debug, Deserialize)]
struct LanguageConnection {
    edges: Vec<Option<LanguageEdge>>,
}

#[derive(Debug, Deserialize)]
struct LanguageEdge {
    size: u64,
    node: LanguageNode,
}

#[derive(Debug, Deserialize)]
struct LanguageNode {
    name: String,
    color: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TopicConnection {
    nodes: Vec<Option<TopicWrapper>>,
}

#[derive(Debug, Deserialize)]
struct TopicWrapper {
    topic: TopicName,
}

#[derive(Debug, Deserialize)]
struct TopicName {
    name: String,
}

#[derive(Debug, Deserialize)]
struct BranchRef {
    target: Option<CommitTarget>,
}

#[derive(Debug, Deserialize)]
struct CommitTarget {
    history: Option<CommitHistory>,
}

#[derive(Debug, Deserialize)]
struct CommitHistory {
    nodes: Vec<Option<CommitNode>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitNode {
    oid: String,
    committed_date: String,
}

fn post_graphql<T: for<'de> Deserialize<'de>>(
    agent: &ureq::Agent,
    token: &str,
    query: &str,
    variables: serde_json::Value,
) -> Result<T> {
    let response = agent
        .post(GITHUB_GRAPHQL_ENDPOINT)
        .set("Authorization", &format!("Bearer {token}"))
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .send_json(json!({
            "query": query,
            "variables": variables,
        }))
        .context("GitHub GraphQL HTTP request failed")?;

    let payload: GraphQlResponse<T> = response
        .into_json()
        .context("failed to decode GitHub GraphQL JSON response")?;

    if let Some(errors) = payload.errors {
        let joined = errors
            .into_iter()
            .map(|e| e.message)
            .collect::<Vec<_>>()
            .join("; ");
        bail!("GitHub GraphQL returned errors: {joined}");
    }

    payload
        .data
        .ok_or_else(|| anyhow!("GitHub GraphQL response missing data payload"))
}

pub fn fetch_live_metrics(token: &str, username: &str, tz_offset_hours: i32) -> Result<MetricsData> {
    let now = Utc::now();
    let from = now - ChronoDuration::days(364);

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(30))
        .build();

    let overview: OverviewData = post_graphql(
        &agent,
        token,
        OVERVIEW_QUERY,
        json!({
            "login": username,
            "from": from.to_rfc3339(),
            "to": now.to_rfc3339(),
        }),
    )?;

    let user = overview
        .user
        .ok_or_else(|| anyhow!("GitHub user '{username}' not found"))?;

    let joined_at = DateTime::parse_from_rfc3339(&user.created_at)
        .context("failed to parse user createdAt timestamp")?
        .with_timezone(&Utc);

    let all_weeks = parse_calendar_weeks(&user.contributions_collection.contribution_calendar);
    let all_days: Vec<&ContributionDay> = all_weeks.iter().flatten().collect();
    let today = now.date_naive();
    let (current_streak, longest_streak) = compute_streaks(&all_days, today);

    let cutoff_30d = today - ChronoDuration::days(RECENT_COMMITS_DAYS - 1);
    let commits_30d: u32 = all_days
        .iter()
        .filter(|d| d.date >= cutoff_30d && d.date <= today)
        .map(|d| d.count)
        .sum();

    let half_year_weeks = take_recent_weeks(&all_weeks, CALENDAR_WEEKS);
    let half_year_days: Vec<&ContributionDay> = half_year_weeks.iter().flatten().collect();
    let half_year_total: u32 = half_year_days.iter().map(|d| d.count).sum();
    let avg_commits_per_day = if half_year_days.is_empty() {
        0.0
    } else {
        f64::from(half_year_total) / half_year_days.len() as f64
    };

    let recent_12_levels: Vec<u8> = all_days
        .iter()
        .rev()
        .take(12)
        .map(|d| d.level)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    // Cursor-paginate through EVERY non-fork repository owned by the user.
    let mut repos: Vec<RepoNode> = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let page: ReposData = post_graphql(
            &agent,
            token,
            REPOSITORIES_QUERY,
            json!({
                "login": &user.login,
                "cursor": cursor,
                "authorId": &user.id,
            }),
        )?;

        let connection = page
            .user
            .ok_or_else(|| anyhow!("GitHub user '{}' disappeared during pagination", user.login))?
            .repositories;

        repos.extend(connection.nodes.into_iter().flatten());

        if connection.page_info.has_next_page && connection.page_info.end_cursor.is_some() {
            cursor = connection.page_info.end_cursor;
        } else {
            break;
        }
    }

    let mut total_stars = 0u32;
    let mut total_forks = 0u32;
    let mut total_releases = 0u32;
    let mut total_disk_kb = 0u64;
    let mut active_repos_90d = 0u32;
    let mut license_counts: HashMap<String, u32> = HashMap::new();
    let mut repo_summaries: Vec<RepoSummary> = Vec::with_capacity(repos.len());
    let mut all_repo_languages: Vec<Vec<RepoLanguageSample>> = Vec::new();
    let mut recent_repo_languages: Vec<Vec<RepoLanguageSample>> = Vec::new();
    let mut seen_oids: HashSet<String> = HashSet::new();
    let mut authored_timestamps: Vec<DateTime<Utc>> = Vec::new();

    let recent_cutoff = now - ChronoDuration::days(RECENT_ACTIVITY_DAYS);

    for repo in &repos {
        total_stars = total_stars.saturating_add(repo.stargazer_count);
        total_forks = total_forks.saturating_add(repo.fork_count);
        total_releases = total_releases.saturating_add(repo.releases.total_count);
        total_disk_kb = total_disk_kb.saturating_add(repo.disk_usage.unwrap_or(0));

        if let Some(license) = &repo.license_info {
            let label = license
                .spdx_id
                .as_deref()
                .filter(|id| !id.is_empty() && *id != "NOASSERTION" && *id != "NONE")
                .unwrap_or(license.name.as_str());
            if !label.is_empty() && label != "Other" && label != "NOASSERTION" {
                *license_counts.entry(label.to_string()).or_insert(0) += 1;
            }
        }

        let pushed_at = repo
            .pushed_at
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));
        let is_recent_repo = pushed_at.is_some_and(|ts| ts >= recent_cutoff);

        if is_recent_repo && !repo.name.eq_ignore_ascii_case(&user.login) {
            active_repos_90d = active_repos_90d.saturating_add(1);
        }

        let lang_samples: Vec<RepoLanguageSample> = repo
            .languages
            .edges
            .iter()
            .flatten()
            .filter(|edge| edge.size > 0)
            .map(|edge| RepoLanguageSample {
                name: edge.node.name.clone(),
                bytes: edge.size,
                color: edge
                    .node
                    .color
                    .clone()
                    .unwrap_or_else(|| "#8b949e".to_string()),
            })
            .collect();

        if !lang_samples.is_empty() {
            if is_recent_repo {
                recent_repo_languages.push(lang_samples.clone());
            }
            all_repo_languages.push(lang_samples.clone());
        }

        let topics: Vec<String> = repo
            .repository_topics
            .nodes
            .iter()
            .flatten()
            .map(|t| t.topic.name.trim().to_ascii_lowercase())
            .filter(|name| !name.is_empty())
            .collect();

        repo_summaries.push(RepoSummary {
            name: repo.name.clone(),
            is_private: repo.is_private,
            stars: repo.stargazer_count,
            pushed_at,
            primary_language: repo.primary_language.as_ref().map(|l| l.name.clone()),
            primary_language_color: repo
                .primary_language
                .as_ref()
                .and_then(|l| l.color.clone()),
            languages: lang_samples,
            topics,
        });

        if let Some(nodes) = repo
            .default_branch_ref
            .as_ref()
            .and_then(|b| b.target.as_ref())
            .and_then(|t| t.history.as_ref())
            .map(|h| &h.nodes)
        {
            for commit in nodes.iter().flatten() {
                if !seen_oids.insert(commit.oid.clone()) {
                    continue;
                }
                if let Ok(ts) = DateTime::parse_from_rfc3339(&commit.committed_date) {
                    authored_timestamps.push(ts.with_timezone(&Utc));
                }
            }
        }
    }

    // Sort newest commits first, cap at 200 most recent authored commits, and bucket in local TZ.
    authored_timestamps.sort_by(|a, b| b.cmp(a));
    authored_timestamps.truncate(MAX_HABIT_COMMITS);
    let (hourly_commits, weekday_commits) =
        bucket_commit_timestamps(&authored_timestamps, tz_offset_hours);

    let preferred_license = license_counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(name, _)| name);

    let languages = aggregate_normalized_languages(&all_repo_languages, TOP_LANGUAGES_LIMIT, true);
    let recent_languages =
        aggregate_normalized_languages(&recent_repo_languages, RECENT_LANGUAGES_LIMIT, false);

    // Strict privacy guards: private repos are NEVER exposed in active repos, topics, or stdout logs.
    let active_repos =
        filter_public_active_repos(&repo_summaries, &user.login, TOP_ACTIVE_REPOS_LIMIT);
    let topics = collect_public_topics(&repo_summaries, TOP_TOPICS_LIMIT);
    let top_public_repos_by_bytes =
        top_public_repos_by_raw_bytes(&repo_summaries, TOP_DIAGNOSTIC_REPOS_LIMIT);

    Ok(MetricsData {
        login: user.login,
        joined_at,
        followers: user.followers.total_count,
        repo_count: repos.len() as u32,
        active_repos_90d,
        total_stars,
        total_forks,
        total_releases,
        total_disk_kb,
        preferred_license,
        contributions_1y: user
            .contributions_collection
            .contribution_calendar
            .total_contributions,
        commits_30d,
        tz_offset_hours,
        hourly_commits,
        weekday_commits,
        calendar_weeks: half_year_weeks,
        recent_12_levels,
        current_streak,
        longest_streak,
        avg_commits_per_day,
        languages,
        recent_languages,
        active_repos,
        topics,
        top_public_repos_by_bytes,
        generated_at: now,
    })
}

/// Computes per-repository normalized language shares.
///
/// Each repository with language bytes contributes a total weight of `1.0`, split across
/// its languages proportionally to its byte breakdown. Summing across repositories keeps a
/// single multi-megabyte repository from drowning out smaller repositories.
pub fn aggregate_normalized_languages(
    repos: &[Vec<RepoLanguageSample>],
    top_limit: usize,
    include_other: bool,
) -> Vec<LanguageShare> {
    let mut totals: HashMap<String, (f64, String)> = HashMap::new();

    for repo_langs in repos {
        let repo_total_bytes: u64 = repo_langs.iter().map(|l| l.bytes).sum();
        if repo_total_bytes == 0 {
            continue;
        }
        let denom = repo_total_bytes as f64;

        for lang in repo_langs {
            if lang.bytes == 0 {
                continue;
            }
            let normalized_share = lang.bytes as f64 / denom;
            let entry = totals
                .entry(lang.name.clone())
                .or_insert_with(|| (0.0, lang.color.clone()));
            entry.0 += normalized_share;
        }
    }

    let mut ranked: Vec<(String, f64, String)> = totals
        .into_iter()
        .map(|(name, (weight, color))| (name, weight, color))
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let grand_total: f64 = ranked.iter().map(|(_, weight, _)| *weight).sum();
    if grand_total <= 0.0 {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut other_weight = 0.0;

    for (idx, (name, weight, color)) in ranked.into_iter().enumerate() {
        if idx < top_limit {
            result.push(LanguageShare {
                name,
                color,
                weight,
                percent: (weight / grand_total) * 100.0,
            });
        } else {
            other_weight += weight;
        }
    }

    if include_other && other_weight > 0.0 {
        result.push(LanguageShare {
            name: "Other".to_string(),
            color: "#8b949e".to_string(),
            weight: other_weight,
            percent: (other_weight / grand_total) * 100.0,
        });
    }

    result
}

/// Strictly filters active repositories to public non-profile repositories.
pub fn filter_public_active_repos(
    repos: &[RepoSummary],
    profile_login: &str,
    limit: usize,
) -> Vec<ActiveRepo> {
    let mut active: Vec<ActiveRepo> = repos
        .iter()
        .filter(|repo| !repo.is_private && !repo.name.eq_ignore_ascii_case(profile_login))
        .filter_map(|repo| {
            let pushed_at = repo.pushed_at?;
            Some(ActiveRepo {
                name: repo.name.clone(),
                primary_language: repo.primary_language.clone(),
                primary_language_color: repo.primary_language_color.clone(),
                stars: repo.stars,
                pushed_at,
            })
        })
        .collect();

    active.sort_by(|a, b| {
        b.pushed_at
            .cmp(&a.pushed_at)
            .then_with(|| b.stars.cmp(&a.stars))
            .then_with(|| a.name.cmp(&b.name))
    });
    active.truncate(limit);
    active
}

/// Strictly aggregates topics from public repositories only.
pub fn collect_public_topics(repos: &[RepoSummary], limit: usize) -> Vec<String> {
    let mut topic_counts: HashMap<String, u32> = HashMap::new();
    for repo in repos.iter().filter(|r| !r.is_private) {
        for topic in &repo.topics {
            if !topic.is_empty() {
                *topic_counts.entry(topic.clone()).or_insert(0) += 1;
            }
        }
    }

    let mut ranked: Vec<(String, u32)> = topic_counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked.truncate(limit);
    ranked.into_iter().map(|(name, _)| name).collect()
}

/// Strictly ranks top public repositories by raw language bytes for CI log diagnostics.
pub fn top_public_repos_by_raw_bytes(
    repos: &[RepoSummary],
    limit: usize,
) -> Vec<RepoSizeDiagnostic> {
    let mut rows: Vec<RepoSizeDiagnostic> = repos
        .iter()
        .filter(|repo| !repo.is_private)
        .map(|repo| {
            let raw_bytes: u64 = repo.languages.iter().map(|l| l.bytes).sum();
            let primary_language = repo
                .primary_language
                .clone()
                .unwrap_or_else(|| "None".to_string());
            RepoSizeDiagnostic {
                name: repo.name.clone(),
                raw_bytes,
                primary_language,
            }
        })
        .collect();

    rows.sort_by(|a, b| {
        b.raw_bytes
            .cmp(&a.raw_bytes)
            .then_with(|| a.name.cmp(&b.name))
    });
    rows.truncate(limit);
    rows
}

pub fn format_raw_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;

    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

pub fn print_repo_size_diagnostics(rows: &[RepoSizeDiagnostic]) {
    println!("Top public repositories by raw language byte size:");
    if rows.is_empty() {
        println!("  (no public repositories found)");
        return;
    }
    for (idx, row) in rows.iter().enumerate() {
        println!(
            "  {}. {:<28} {:>10}  ({})",
            idx + 1,
            row.name,
            format_raw_bytes(row.raw_bytes),
            row.primary_language
        );
    }
}

fn parse_contribution_level(level: &str, count: u32) -> u8 {
    match level {
        "NONE" => 0,
        "FIRST_QUARTILE" => 1,
        "SECOND_QUARTILE" => 2,
        "THIRD_QUARTILE" => 3,
        "FOURTH_QUARTILE" => 4,
        _ => match count {
            0 => 0,
            1..=2 => 1,
            3..=5 => 2,
            6..=9 => 3,
            _ => 4,
        },
    }
}

fn parse_calendar_weeks(calendar: &ContributionCalendarNode) -> Vec<Vec<ContributionDay>> {
    calendar
        .weeks
        .iter()
        .map(|week| {
            week.contribution_days
                .iter()
                .filter_map(|d| {
                    let date = NaiveDate::parse_from_str(&d.date, "%Y-%m-%d").ok()?;
                    Some(ContributionDay {
                        date,
                        count: d.contribution_count,
                        level: parse_contribution_level(&d.contribution_level, d.contribution_count),
                    })
                })
                .collect()
        })
        .filter(|w: &Vec<ContributionDay>| !w.is_empty())
        .collect()
}

fn take_recent_weeks(
    weeks: &[Vec<ContributionDay>],
    max_weeks: usize,
) -> Vec<Vec<ContributionDay>> {
    let start = weeks.len().saturating_sub(max_weeks);
    weeks[start..].to_vec()
}

pub fn compute_streaks(days: &[&ContributionDay], today: NaiveDate) -> (u32, u32) {
    if days.is_empty() {
        return (0, 0);
    }

    let mut longest = 0u32;
    let mut run = 0u32;
    let mut prev_date: Option<NaiveDate> = None;

    for day in days {
        let consecutive = prev_date.is_some_and(|p| day.date == p + ChronoDuration::days(1));
        if day.count > 0 {
            if consecutive || run == 0 {
                run += 1;
            } else {
                run = 1;
            }
            if run > longest {
                longest = run;
            }
        } else {
            run = 0;
        }
        prev_date = Some(day.date);
    }

    // Current streak counts consecutive active days ending today, or yesterday if today is 0.
    let mut by_date: HashMap<NaiveDate, u32> = HashMap::with_capacity(days.len());
    for day in days {
        by_date.insert(day.date, day.count);
    }

    let mut cursor = today;
    if by_date.get(&cursor).copied().unwrap_or(0) == 0 {
        cursor -= ChronoDuration::days(1);
    }

    let mut current = 0u32;
    while by_date.get(&cursor).copied().unwrap_or(0) > 0 {
        current += 1;
        cursor -= ChronoDuration::days(1);
    }

    (current, longest)
}

pub fn bucket_commit_timestamps(
    timestamps: &[DateTime<Utc>],
    tz_offset_hours: i32,
) -> ([u32; 24], [u32; 7]) {
    let offset = FixedOffset::east_opt(tz_offset_hours * 3600)
        .unwrap_or_else(|| FixedOffset::east_opt(0).expect("UTC offset valid"));

    let mut hourly = [0u32; 24];
    let mut weekday = [0u32; 7];

    for &ts in timestamps {
        let local = ts.with_timezone(&offset);
        let hour = local.hour() as usize;
        let day = local.weekday().num_days_from_sunday() as usize;
        if hour < 24 {
            hourly[hour] = hourly[hour].saturating_add(1);
        }
        if day < 7 {
            weekday[day] = weekday[day].saturating_add(1);
        }
    }

    (hourly, weekday)
}

/// Deterministic offline mock dataset for layout inspection (`--mock`).
pub fn mock_metrics(username: &str, tz_offset_hours: i32) -> MetricsData {
    let now = Utc::now();
    let today = now.date_naive();

    // Generate 26 deterministic weeks with organic burst distribution for realistic preview.
    let total_days = CALENDAR_WEEKS * 7;
    let start_date = today - ChronoDuration::days((total_days - 1) as i64);
    let mut weeks: Vec<Vec<ContributionDay>> = Vec::with_capacity(CALENDAR_WEEKS);
    let mut current_week: Vec<ContributionDay> = Vec::with_capacity(7);
    let mut seed: u64 = 0x5A17_C0DE_1234_9876;

    for i in 0..total_days {
        let date = start_date + ChronoDuration::days(i as i64);
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let r = ((seed >> 33) % 100) as u32;

        let count = if (48..67).contains(&i) {
            1 + (r % 9)
        } else if i >= total_days - 8 {
            1 + (r % 7)
        } else if r < 34 {
            0
        } else if r < 62 {
            1 + (r % 2)
        } else if r < 82 {
            3 + (r % 3)
        } else if r < 94 {
            6 + (r % 4)
        } else {
            10 + (r % 6)
        };
        let level = match count {
            0 => 0,
            1..=2 => 1,
            3..=5 => 2,
            6..=9 => 3,
            _ => 4,
        };

        current_week.push(ContributionDay { date, count, level });
        if current_week.len() == 7 {
            weeks.push(current_week);
            current_week = Vec::with_capacity(7);
        }
    }
    if !current_week.is_empty() {
        weeks.push(current_week);
    }

    let flat_days: Vec<&ContributionDay> = weeks.iter().flatten().collect();
    let (current_streak, longest_streak) = compute_streaks(&flat_days, today);
    let total_half_year: u32 = flat_days.iter().map(|d| d.count).sum();
    let avg_commits_per_day = f64::from(total_half_year) / flat_days.len().max(1) as f64;
    let cutoff_30d = today - ChronoDuration::days(RECENT_COMMITS_DAYS - 1);
    let commits_30d: u32 = flat_days
        .iter()
        .filter(|d| d.date >= cutoff_30d && d.date <= today)
        .map(|d| d.count)
        .sum();
    let recent_12_levels: Vec<u8> = flat_days
        .iter()
        .rev()
        .take(12)
        .map(|d| d.level)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let hourly_commits = [
        7, 4, 2, 1, 0, 0, 0, 1, 3, 6, 9, 11, 8, 12, 14, 13, 10, 8, 6, 9, 15, 18, 16, 11,
    ];
    let weekday_commits = [22, 28, 31, 26, 29, 25, 23];

    let languages = vec![
        LanguageShare {
            name: "Rust".to_string(),
            color: "#dea584".to_string(),
            weight: 4.21,
            percent: 42.1,
        },
        LanguageShare {
            name: "TypeScript".to_string(),
            color: "#3178c6".to_string(),
            weight: 2.48,
            percent: 24.8,
        },
        LanguageShare {
            name: "Java".to_string(),
            color: "#b07219".to_string(),
            weight: 1.12,
            percent: 11.2,
        },
        LanguageShare {
            name: "Dart".to_string(),
            color: "#00b4ab".to_string(),
            weight: 0.68,
            percent: 6.8,
        },
        LanguageShare {
            name: "Python".to_string(),
            color: "#3572a5".to_string(),
            weight: 0.46,
            percent: 4.6,
        },
        LanguageShare {
            name: "C".to_string(),
            color: "#555555".to_string(),
            weight: 0.34,
            percent: 3.4,
        },
        LanguageShare {
            name: "HTML".to_string(),
            color: "#e34c26".to_string(),
            weight: 0.28,
            percent: 2.8,
        },
        LanguageShare {
            name: "Shell".to_string(),
            color: "#89e051".to_string(),
            weight: 0.21,
            percent: 2.1,
        },
        LanguageShare {
            name: "Other".to_string(),
            color: "#8b949e".to_string(),
            weight: 0.22,
            percent: 2.2,
        },
    ];

    let recent_languages = vec![
        LanguageShare {
            name: "Rust".to_string(),
            color: "#dea584".to_string(),
            weight: 2.456,
            percent: 61.4,
        },
        LanguageShare {
            name: "TypeScript".to_string(),
            color: "#3178c6".to_string(),
            weight: 0.912,
            percent: 22.8,
        },
        LanguageShare {
            name: "HTML".to_string(),
            color: "#e34c26".to_string(),
            weight: 0.376,
            percent: 9.4,
        },
        LanguageShare {
            name: "Shell".to_string(),
            color: "#89e051".to_string(),
            weight: 0.256,
            percent: 6.4,
        },
    ];

    let active_repos = vec![
        ActiveRepo {
            name: "rustboy-advance".to_string(),
            primary_language: Some("Rust".to_string()),
            primary_language_color: Some("#dea584".to_string()),
            stars: 18,
            pushed_at: now - ChronoDuration::hours(3),
        },
        ActiveRepo {
            name: "logpulse".to_string(),
            primary_language: Some("Rust".to_string()),
            primary_language_color: Some("#dea584".to_string()),
            stars: 12,
            pushed_at: now - ChronoDuration::days(2),
        },
        ActiveRepo {
            name: "hearthsand".to_string(),
            primary_language: Some("Rust".to_string()),
            primary_language_color: Some("#dea584".to_string()),
            stars: 9,
            pushed_at: now - ChronoDuration::days(5),
        },
        ActiveRepo {
            name: "heartconnect".to_string(),
            primary_language: Some("TypeScript".to_string()),
            primary_language_color: Some("#3178c6".to_string()),
            stars: 7,
            pushed_at: now - ChronoDuration::days(11),
        },
    ];

    let topics = vec![
        "rust".to_string(),
        "emulator".to_string(),
        "gba".to_string(),
        "cli".to_string(),
        "tui".to_string(),
        "typescript".to_string(),
        "simulation".to_string(),
        "systems".to_string(),
    ];

    let top_public_repos_by_bytes = vec![
        RepoSizeDiagnostic {
            name: "rustboy-advance".to_string(),
            raw_bytes: 1_480_000,
            primary_language: "Rust".to_string(),
        },
        RepoSizeDiagnostic {
            name: "heartconnect".to_string(),
            raw_bytes: 820_000,
            primary_language: "TypeScript".to_string(),
        },
        RepoSizeDiagnostic {
            name: "hearthsand".to_string(),
            raw_bytes: 540_000,
            primary_language: "Rust".to_string(),
        },
        RepoSizeDiagnostic {
            name: "logpulse".to_string(),
            raw_bytes: 310_000,
            primary_language: "Rust".to_string(),
        },
        RepoSizeDiagnostic {
            name: "b3dup".to_string(),
            raw_bytes: 145_000,
            primary_language: "Rust".to_string(),
        },
    ];

    MetricsData {
        login: username.to_string(),
        joined_at: now - ChronoDuration::days(365 * 4 + 90),
        followers: 24,
        repo_count: 28,
        active_repos_90d: 6,
        total_stars: 56,
        total_forks: 11,
        total_releases: 9,
        total_disk_kb: 74_820,
        preferred_license: Some("MIT".to_string()),
        contributions_1y: 684,
        commits_30d,
        tz_offset_hours,
        hourly_commits,
        weekday_commits,
        calendar_weeks: weeks,
        recent_12_levels,
        current_streak,
        longest_streak,
        avg_commits_per_day,
        languages,
        recent_languages,
        active_repos,
        topics,
        top_public_repos_by_bytes,
        generated_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn normalized_language_weighting_prevents_large_repo_skew() {
        // 1 huge 50 MB C# repo vs 4 lean 100 KB Rust repos.
        let repos = vec![
            vec![RepoLanguageSample {
                name: "C#".to_string(),
                bytes: 50_000_000,
                color: "#178600".to_string(),
            }],
            vec![RepoLanguageSample {
                name: "Rust".to_string(),
                bytes: 100_000,
                color: "#dea584".to_string(),
            }],
            vec![RepoLanguageSample {
                name: "Rust".to_string(),
                bytes: 100_000,
                color: "#dea584".to_string(),
            }],
            vec![RepoLanguageSample {
                name: "Rust".to_string(),
                bytes: 100_000,
                color: "#dea584".to_string(),
            }],
            vec![RepoLanguageSample {
                name: "Rust".to_string(),
                bytes: 100_000,
                color: "#dea584".to_string(),
            }],
        ];

        let shares = aggregate_normalized_languages(&repos, 8, true);
        assert_eq!(shares.len(), 2);
        assert_eq!(shares[0].name, "Rust");
        assert!((shares[0].percent - 80.0).abs() < 1e-6);
        assert_eq!(shares[1].name, "C#");
        assert!((shares[1].percent - 20.0).abs() < 1e-6);
    }

    #[test]
    fn normalized_language_weighting_splits_mixed_repo_shares_to_one() {
        // Repo 1: 80% Rust / 20% Shell -> contributes 0.8 Rust, 0.2 Shell.
        // Repo 2: 100% TypeScript -> contributes 1.0 TypeScript.
        let repos = vec![
            vec![
                RepoLanguageSample {
                    name: "Rust".to_string(),
                    bytes: 800,
                    color: "#dea584".to_string(),
                },
                RepoLanguageSample {
                    name: "Shell".to_string(),
                    bytes: 200,
                    color: "#89e051".to_string(),
                },
            ],
            vec![RepoLanguageSample {
                name: "TypeScript".to_string(),
                bytes: 500_000,
                color: "#3178c6".to_string(),
            }],
        ];

        let shares = aggregate_normalized_languages(&repos, 8, true);
        assert_eq!(shares.len(), 3);
        assert_eq!(shares[0].name, "TypeScript");
        assert!((shares[0].percent - 50.0).abs() < 1e-6);
        assert_eq!(shares[1].name, "Rust");
        assert!((shares[1].percent - 40.0).abs() < 1e-6);
        assert_eq!(shares[2].name, "Shell");
        assert!((shares[2].percent - 10.0).abs() < 1e-6);
    }

    #[test]
    fn privacy_guard_excludes_private_repos_from_active_topics_and_diagnostics() {
        let t1 = Utc.with_ymd_and_hms(2026, 11, 19, 10, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2026, 11, 18, 10, 0, 0).unwrap();

        let repos = vec![
            RepoSummary {
                name: "secret-client-app".to_string(),
                is_private: true,
                stars: 99,
                pushed_at: Some(t1),
                primary_language: Some("C#".to_string()),
                primary_language_color: Some("#178600".to_string()),
                languages: vec![RepoLanguageSample {
                    name: "C#".to_string(),
                    bytes: 90_000_000,
                    color: "#178600".to_string(),
                }],
                topics: vec!["confidential".to_string(), "internal".to_string()],
            },
            RepoSummary {
                name: "jpmasangkay".to_string(),
                is_private: false,
                stars: 1,
                pushed_at: Some(t1),
                primary_language: Some("Rust".to_string()),
                primary_language_color: Some("#dea584".to_string()),
                languages: vec![RepoLanguageSample {
                    name: "Rust".to_string(),
                    bytes: 25_000,
                    color: "#dea584".to_string(),
                }],
                topics: vec![],
            },
            RepoSummary {
                name: "rustboy-advance".to_string(),
                is_private: false,
                stars: 12,
                pushed_at: Some(t2),
                primary_language: Some("Rust".to_string()),
                primary_language_color: Some("#dea584".to_string()),
                languages: vec![RepoLanguageSample {
                    name: "Rust".to_string(),
                    bytes: 400_000,
                    color: "#dea584".to_string(),
                }],
                topics: vec!["rust".to_string(), "emulator".to_string()],
            },
        ];

        let active = filter_public_active_repos(&repos, "jpmasangkay", 4);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "rustboy-advance");

        let topics = collect_public_topics(&repos, 10);
        assert_eq!(topics, vec!["emulator", "rust"]);
        assert!(!topics.iter().any(|t| t == "confidential" || t == "internal"));

        let diag = top_public_repos_by_raw_bytes(&repos, 5);
        assert!(diag.iter().all(|d| d.name != "secret-client-app"));
        assert_eq!(diag[0].name, "rustboy-advance");
    }
}
