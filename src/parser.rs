use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Mutex, OnceLock},
};
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parsed {
    pub title: String,
    #[serde(default)]
    pub alternate_titles: Vec<String>,
    #[serde(default)]
    pub collection_title: Option<String>,
    pub episode_title: String,
    pub season: Option<u32>,
    pub episodes: Vec<u32>,
    pub year: Option<u32>,
    pub kind: String,
    pub explanation: String,
}
fn re(s: &str) -> Regex {
    static CACHE: OnceLock<Mutex<HashMap<String, Regex>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap();
    cache
        .entry(s.to_owned())
        .or_insert_with(|| Regex::new(s).unwrap())
        .clone()
}
pub fn normalize(s: &str) -> String {
    let s: String = s
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase();
    re(r"[^\p{L}\p{N}]+")
        .replace_all(&s, " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn clean(s: &str) -> String {
    let s = re(r"(?i)(?:[ ._\-\[(]+)(?:[0-9]{3,4}x[0-9]{3,4}|2160p|1080p|720p|480p|360p|4k|bdrip|sl(?:o|v)subs?|engsubs?|blu[ .-]?ray|web[ .-]?(?:dl|rip)|hdtv|x26[45]|h[ .]?26[45]|hevc|av1|hdr10?|dolby[ .]?vision|aac|dts|atmos|remux|proper|repack|extended)\b.*$").replace(s, "");
    s.replace(['.', '_'], " ")
        .trim_matches(|c: char| c.is_whitespace() || "-[]()".contains(c))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
// Titles such as "Blade Runner 2049 (2017)" or "2001 A Space Odyssey (1968)"
// contain year-like numbers. Prefer a bracketed year, otherwise the last one.
fn release_year(text: &str) -> Option<regex::Match<'_>> {
    let years: Vec<_> = re(r"\b(?:19|20)[0-9]{2}\b").find_iter(text).collect();
    years
        .iter()
        .rev()
        .find(|m| text[..m.start()].ends_with(['(', '[']) && text[m.end()..].starts_with([')', ']']))
        .or(years.last())
        .copied()
}
pub fn parse(path: &Path) -> Parsed {
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let parent = path
        .parent()
        .and_then(|p| p.file_name())
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let grand = path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let pattern = re(
        r"(?i)\b(?:s(?:eason)?[ ._-]*0*([0-9]{1,2})[ ._-]*(?:e|ep(?:isode)?)[ ._-]*0*([0-9]{1,3})|0*([0-9]{1,2})x0*([0-9]{1,3}))",
    );
    let season_re = re(r"(?i)\b(?:s|season)[ ._-]*0*([0-9]{1,2})\b");
    let ep_re = re(r"(?i)\b(?:e|ep|episode)[ ._-]*0*([0-9]{1,3})\b");
    let year_re = re(r"\b(19[0-9]{2}|20[0-9]{2})\b");
    let season;
    let mut episodes = vec![];
    let mut prefix = stem.as_str();
    let mut suffix = "";
    let special = re(r"(?i)\bs[0-9]{1,2}x[0-9]{1,3}\b").is_match(&stem);
    if let Some(c) = pattern.captures(&stem).filter(|_| !special) {
        let m = c.get(0).unwrap();
        prefix = &stem[..m.start()];
        suffix = &stem[m.end()..];
        season = c
            .get(1)
            .or_else(|| c.get(3))
            .and_then(|m| m.as_str().parse().ok());
        if let Some(n) = c
            .get(2)
            .or_else(|| c.get(4))
            .and_then(|m| m.as_str().parse().ok())
        {
            episodes.push(n);
        }
        let extra = re(r"(?i)^(?:[ ._+]*(?:e|ep)[ ._]*0*[0-9]{1,3})+");
        if let Some(m) = extra.find(suffix) {
            for c in re(r"(?i)(?:e|ep)[ ._-]*0*([0-9]{1,3})").captures_iter(m.as_str()) {
                if let Ok(n) = c[1].parse() {
                    episodes.push(n)
                }
            }
            suffix = &suffix[m.end()..];
        }
        // A bare number is a range end only when attached ("E01-03"); after
        // spaces it is the start of a title such as "S01E02 - 7 Days Later".
        let range = re(r"(?i)^(?:\s*-[ ._]*(?:e|ep)0*([0-9]{1,3})|-0*([0-9]{1,3}))\b");
        if let Some(c) = range.captures(suffix) {
            let end = c.get(1).or_else(|| c.get(2)).map(|m| m.as_str().parse::<u32>());
            if let (Some(&start), Some(Ok(end))) = (episodes.first(), end) {
                if end > start && end - start <= 20 {
                    episodes.extend(start + 1..=end);
                }
            }
            suffix = &suffix[c.get(0).unwrap().end()..];
        }
    } else {
        season = season_re.captures(&parent).and_then(|c| c[1].parse().ok());
        if season.is_some() {
            if let Some(c) = ep_re.captures(&stem) {
                episodes.push(c[1].parse().unwrap_or(0));
                prefix = &stem[..c.get(0).unwrap().start()];
                suffix = &stem[c.get(0).unwrap().end()..];
            }
            if episodes.is_empty() {
                // Compact SxxEyy represented as 714 or 0714, only if the
                // parent establishes the same season.
                let compact = re(r"\b([0-9]{3,4})\b");
                if let Some(c) = compact.captures(&stem) {
                    let digits = &c[1];
                    let split = digits.len() - 2;
                    let sn = digits[..split].parse::<u32>().ok();
                    let en = digits[split..].parse::<u32>().ok();
                    if sn == season && en.is_some_and(|n| n > 0) {
                        episodes.push(en.unwrap());
                        let m = c.get(0).unwrap();
                        prefix = &stem[..m.start()];
                        suffix = &stem[m.end()..];
                    }
                }
            }
        }
    }
    let stem_year = release_year(&stem);
    let year = stem_year
        .map(|m| m.as_str().to_owned())
        .or_else(|| year_re.captures(&parent).map(|c| c[1].to_owned()))
        .or_else(|| year_re.captures(&grand).map(|c| c[1].to_owned()))
        .and_then(|y| y.parse().ok());
    // Names of bonus folders are not reliable show titles. Only treat an
    // Extras folder as a bonus folder when it sits beneath a specific owner,
    // so a real show named "Extras" under a generic library is not discarded.
    let bonus_name = re(
        r"(?i)\b(sample|trailer|featurette|behind[ ._-]the[ ._-]scenes|deleted[ ._-]scenes?|gag[ ._-]reel|alternate[ ._-](?:ending|opening)|webisodes?)\b",
    );
    let generic_container = |s: &str| {
        matches!(
            normalize(s).as_str(),
            "media"
                | "movies"
                | "tv"
                | "tv shows"
                | "shows"
                | "series"
                | "video"
                | "videos"
                | "library"
        )
    };
    let bonus_folder = path
        .parent()
        .into_iter()
        .flat_map(|p| p.ancestors())
        .any(|folder| {
            let name = folder.file_name().unwrap_or_default().to_string_lossy();
            let n = normalize(&name);
            if !matches!(
                n.as_str(),
                "extras"
                    | "extra"
                    | "bonus"
                    | "bonus features"
                    | "featurettes"
                    | "webisodes"
                    | "deleted scenes"
                    | "behind the scenes"
            ) {
                return false;
            }
            let owner = folder
                .parent()
                .and_then(|p| p.file_name())
                .unwrap_or_default()
                .to_string_lossy();
            !owner.is_empty() && !generic_container(&owner)
        });
    let extra = special || bonus_folder || (episodes.is_empty() && bonus_name.is_match(&stem));
    let collection = re(
        r"(?i)[ ._-]+(?:complete(?:[ ._-]+classic)?[ ._-]+collection|season[ ._-]+[0-9]+[ ._-]+to[ ._-]+[0-9]+).*$",
    );
    let region = re(r"(?i)\s*\((?:us|uk)\)");
    let folder_title =
        clean(&region.replace(&collection.replace(&season_re.replace(&parent, ""), ""), ""));
    let folder_title = if folder_title.is_empty() {
        clean(&grand)
    } else {
        folder_title
    };
    let raw_title = if !episodes.is_empty() {
        let c = clean(&region.replace(&year_re.replace(prefix, ""), ""));
        if c.is_empty() {
            folder_title.clone()
        } else {
            c
        }
    } else if let Some(year_match) = stem_year {
        let before = clean(&stem[..year_match.start()]);
        if before.is_empty() {
            clean(&stem[year_match.end()..])
        } else {
            before
        }
    } else {
        folder_title.clone()
    };
    let title = if raw_title.is_empty() {
        clean(&stem)
    } else {
        raw_title
    };
    let episode_title = if !episodes.is_empty() {
        let value = clean(suffix);
        if re(r"(?i)^episode[ ._-]*0*[0-9]+$").is_match(&value) {
            String::new()
        } else {
            value
        }
    } else if !year_re.is_match(&stem) {
        let value = clean(&re(r"^\s*[0-9]{1,4}[ ._-]+\s*").replace(&stem, ""));
        let title_norm = normalize(&title);
        if normalize(&value).starts_with(&(title_norm.clone() + " ")) {
            let words = title.split_whitespace().count();
            value
                .split_whitespace()
                .skip(words)
                .collect::<Vec<_>>()
                .join(" ")
                .trim_matches(|c: char| "-: ".contains(c))
                .to_owned()
        } else {
            value
        }
    } else {
        String::new()
    };
    let short_folder = normalize(&folder_title);
    let long_title = normalize(&title);
    let alternate_titles = if short_folder.len() >= 5
        && !generic_container(&folder_title)
        && short_folder != long_title
        && long_title.starts_with(&(short_folder + " "))
    {
        vec![folder_title]
    } else {
        vec![]
    };
    let collection_title = if episodes.is_empty()
        && re(r"(?i)\bcomplete(?:[ ._-]+classic)?[ ._-]+collection\b").is_match(&parent)
        && re(r"^\s*[0-9]{1,3}[ ._-]+").is_match(&stem)
        && !extra
    {
        Some(title.clone())
    } else {
        None
    };
    Parsed {
        title,
        alternate_titles,
        collection_title,
        episode_title,
        season,
        episodes,
        year,
        kind: if extra {
            "extra"
        } else if season.is_some() || !year_re.is_match(&stem) {
            "tv"
        } else {
            "movie"
        }
        .into(),
        explanation: if extra {
            "Possible extra; excluded from automatic episode matching"
        } else {
            "Parsed filename and folder context; provider verification required"
        }
        .into(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numbered_and_ranges() {
        let p = parse(Path::new("/Media/Rick and morty s1/S01E01-E03.mkv"));
        assert_eq!(p.title, "Rick and morty");
        assert_eq!(p.episodes, vec![1, 2, 3]);
        let p = parse(Path::new(
            "/Media/Suits/Season 05/Suits - S05E16 - 25th Hour.mkv",
        ));
        assert_eq!(p.episodes, vec![16]);
        assert_eq!(p.episode_title, "25th Hour");
    }
    #[test]
    fn movie() {
        let p = parse(Path::new("/Movies/The.Matrix.1999.1080p.BluRay.mkv"));
        assert_eq!(p.title, "The Matrix");
        assert_eq!(p.kind, "movie");
        let p = parse(Path::new(
            "/Media/Collection/1981.Raiders.Of.The.Lost.Ark.1920x816.BDRip.x264.mkv",
        ));
        assert_eq!(p.kind, "movie");
        assert_eq!(p.title, "Raiders Of The Lost Ark");
        assert_eq!(p.year, Some(1981));
    }
    #[test]
    fn title_only() {
        let p = parse(Path::new("/Media/Regular Show/The Power.mkv"));
        assert_eq!(p.title, "Regular Show");
        assert_eq!(p.episode_title, "The Power");
    }
    #[test]
    fn bonus_folders_are_not_show_names() {
        for path in [
            "/Media/Suits (2011)/Extras/Season 06/Gag Reel.mkv",
            "/Media/Suits/Extras/Season 03/E07 Alternate Ending.mkv",
            "/Media/Breaking Bad/Season 02/Webisodes/Breaking.Bad.Webisode.05.mkv",
            "/Media/Another Show/Extras/Season 1/Interview.mkv",
        ] {
            assert_eq!(parse(Path::new(path)).kind, "extra", "{path}");
        }
        let real = parse(Path::new("/Media/Extras/Season 01/Extras.S01E01.mkv"));
        assert_eq!(real.kind, "tv");
        assert_eq!(real.title, "Extras");
        let real = parse(Path::new("/Media/Episodes/Season 01/Episodes.S01E01.mkv"));
        assert_eq!(real.kind, "tv");
        assert_eq!(real.title, "Episodes");
    }
    #[test]
    fn non_ascii_digits_are_ignored_and_never_panic() {
        for name in [
            "Show - ７１４ - Title.mkv",
            "Show S１E２.mkv",
            "Show E７.mkv",
            "٧١٤ Show.mkv",
            "Show ２０１９.mkv",
        ] {
            let p = parse(Path::new(&format!("/Media/Show/Season 7/{name}")));
            assert!(!p.episodes.contains(&0), "{name}: {:?}", p.episodes);
        }
    }
    #[test]
    fn parser_survives_pseudo_random_unicode_names() {
        let alphabet: Vec<char> = "aZ09 ._-+()[]ExSsé世٣７\u{301}'’&".chars().collect();
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..600 {
            let mut name = String::new();
            for _ in 0..(seed % 40) {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                name.push(alphabet[(seed >> 33) as usize % alphabet.len()]);
            }
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            for dir in ["/Media/Show/Season 3", "/Media/Show (2011)", "/"] {
                parse(Path::new(&format!("{dir}/{name}.mkv")));
            }
        }
    }
    #[test]
    fn numeric_title_after_episode_is_not_a_range() {
        let p = parse(Path::new("/Media/Show/Season 01/Show S01E02 - 7 Days Later.mkv"));
        assert_eq!(p.episodes, vec![2]);
        assert_eq!(p.episode_title, "7 Days Later");
        for name in ["Show S01E01-03.mkv", "Show S01E01-E03.mkv", "Show S01E01 - E03.mkv"] {
            let p = parse(Path::new(&format!("/Media/Show/Season 01/{name}")));
            assert_eq!(p.episodes, vec![1, 2, 3], "{name}");
        }
    }
    #[test]
    fn year_like_titles_use_the_release_year() {
        for (path, title, year) in [
            ("/Movies/Blade.Runner.2049.2017.1080p.BluRay.mkv", "Blade Runner 2049", 2017),
            ("/Movies/Blade Runner 2049 (2017) 1080p.mkv", "Blade Runner 2049", 2017),
            ("/Movies/2001 A Space Odyssey (1968).mkv", "2001 A Space Odyssey", 1968),
            ("/Movies/1917.2019.1080p.mkv", "1917", 2019),
        ] {
            let p = parse(Path::new(path));
            assert_eq!((p.title.as_str(), p.year, p.kind.as_str()), (title, Some(year), "movie"), "{path}");
        }
    }
    #[test]
    fn practical_library_patterns() {
        let p = parse(Path::new("/Media/Suits (2011)/Season 09/Suits (2011) - S09E08 - Prisoner's Dilemma (1080p BluRay x265).mkv"));
        let shameless = parse(Path::new(
            "/Media/Shameless (US) (2011)/Season 07/Shameless (US) (2011) - S07E09 - Ouroboros.mkv",
        ));
        assert_eq!(shameless.title, "Shameless");
        assert_eq!(shameless.year, Some(2011));
        assert_eq!(p.title, "Suits");
        assert_eq!(p.year, Some(2011));
        assert_eq!(p.episode_title, "Prisoner's Dilemma");
        let p = parse(Path::new(
            "/Media/Family Guy/Season 7/Family Guy - 714 - We Love You Conrad.avi",
        ));
        assert_eq!(p.title, "Family Guy");
        assert_eq!(p.episodes, vec![14]);
        assert_eq!(p.episode_title, "We Love You Conrad");
        let p = parse(Path::new("/Media/Regular Show/Season 08/Regular Show S08E11+E12 Can You Ear Me Now + Stuck In an Elevator.mp4"));
        assert_eq!(p.episodes, vec![11, 12]);
        let p = parse(Path::new("/Media/Regular Show/Season 04/Regular Show S04E27+E28 Cool Cubed + Trailer Trashed.mp4"));
        assert_eq!(p.kind, "tv");
        assert_eq!(p.episodes, vec![27, 28]);
        let p = parse(Path::new(
            "/Media/Mocvara/Mocvara.S01E02.SLOSubs.1080p.WEBRip.mkv",
        ));
        assert_eq!(p.episode_title, "");
        let p = parse(Path::new("/Media/Tom and Jerry - Complete Classic Collection - All 161 Episodes/029 - The Cat Concerto.mp4"));
        assert_eq!(p.title, "Tom and Jerry");
        assert_eq!(p.episode_title, "The Cat Concerto");
        let p = parse(Path::new(
            "/Media/Adventure Time/Season 07/Adventure Time S07X01 Frog Seasons Spring.mp4",
        ));
        assert_eq!(p.kind, "extra");
    }
}
