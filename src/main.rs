//! An implementation of [tldr](https://github.com/tldr-pages/tldr) in Rust.
//
// Copyright (c) 2015-2020 tealdeer developers
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be
// copied, modified, or distributed except according to those terms.

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::similar_names)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::too_many_lines)]

use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::iter;
use std::path::{Path, PathBuf};
use std::process;
use std::{env, io::Write};

use ansi_term::{Color, Style};
use app_dirs::AppInfo;
use atty::Stream;
use docopt::Docopt;
#[cfg(not(target_os = "windows"))]
use pager::Pager;
use serde_derive::Deserialize;

mod cache;
mod config;
mod dedup;
mod error;
mod formatter;
mod tokenizer;
mod types;

use crate::cache::{Cache, PageLookupResult};
use crate::config::{get_config_dir, get_config_path, make_default_config, Config, MAX_CACHE_AGE};
use crate::dedup::Dedup;
use crate::error::TealdeerError::ConfigError;
use crate::formatter::print_lines;
use crate::tokenizer::Tokenizer;
use crate::types::{ColorOptions, OsType};

const NAME: &str = "tealdeer";
const APP_INFO: AppInfo = AppInfo {
    name: NAME,
    author: NAME,
};
const VERSION: &str = env!("CARGO_PKG_VERSION");
const USAGE: &str = include_str!("usage.docopt");
const ARCHIVE_URL: &str = "https://github.com/tldr-pages/tldr/archive/master.tar.gz";
#[cfg(not(target_os = "windows"))]
const PAGER_COMMAND: &str = "less -R";

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Deserialize)]
struct Args {
    arg_command: Option<Vec<String>>,
    flag_help: bool,
    flag_version: bool,
    flag_list: bool,
    flag_render: Option<String>,
    flag_os: Option<OsType>,
    flag_update: bool,
    flag_clear_cache: bool,
    flag_pager: bool,
    flag_quiet: bool,
    flag_show_paths: bool,
    flag_config_path: bool,
    flag_seed_config: bool,
    flag_markdown: bool,
    flag_color: ColorOptions,
    flag_language: Option<String>,
}

/// Print page by path
fn print_page(
    page: &PageLookupResult,
    enable_markdown: bool,
    config: &Config,
) -> Result<(), String> {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    for path in page.paths() {
        let file = File::open(path).map_err(|msg| format!("Could not open file: {}", msg))?;
        let reader = BufReader::new(file);

        if enable_markdown {
            // Print the raw markdown of the file.
            for line in reader.lines() {
                writeln!(handle, "{}", line.unwrap())
                    .map_err(|_| "Could not write to stdout".to_string())?;
            }
        } else {
            // Create tokenizer and print output
            let mut tokenizer = Tokenizer::new(reader);
            print_lines(&mut handle, &mut tokenizer, &config)
                .map_err(|e| format!("Could not write to stdout: {}", e.message()))?;
        };
    }

    handle
        .flush()
        .map_err(|_| "Could not flush stdout".to_string())?;

    Ok(())
}

/// Set up display pager
#[cfg(not(target_os = "windows"))]
fn configure_pager() {
    Pager::with_default_pager(PAGER_COMMAND).setup();
}

#[cfg(target_os = "windows")]
fn configure_pager() {
    eprintln!("Warning: -p / --pager flag not available on Windows!");
}

fn should_update_cache(args: &Args, config: &Config) -> bool {
    args.flag_update
        || (config.updates.auto_update
            && Cache::last_update().map_or(true, |ago| ago >= config.updates.auto_update_interval))
}

/// Check the cache for freshness
fn check_cache(args: &Args, enable_styles: bool) {
    match Cache::last_update() {
        Some(ago) if ago > MAX_CACHE_AGE => {
            if args.flag_quiet {
                return;
            }

            // Only use color if enabled
            let warning_style = if enable_styles {
                Style::new().fg(Color::Yellow)
            } else {
                Style::default()
            };

            eprintln!(
                "{}",
                warning_style.paint(format!(
                    "The cache hasn't been updated for more than {} days.\n\
                         You should probably run `tldr --update` soon.",
                    MAX_CACHE_AGE.as_secs() / 24 / 3600
                ))
            );
        }
        Some(_) => {}
        None => {
            eprintln!("Cache not found. Please run `tldr --update`.");
            process::exit(1);
        }
    };
}

/// Clear the cache
fn clear_cache(quietly: bool) {
    Cache::clear().unwrap_or_else(|e| {
        eprintln!("Could not delete cache: {}", e.message());
        process::exit(1);
    });
    if !quietly {
        eprintln!("Successfully deleted cache.");
    }
}

/// Update the cache
fn update_cache(cache: &Cache, quietly: bool) {
    cache.update().unwrap_or_else(|e| {
        eprintln!("Could not update cache: {}", e.message());
        process::exit(1);
    });
    if !quietly {
        eprintln!("Successfully updated cache.");
    }
}

/// Show the config path (DEPRECATED)
fn show_config_path() {
    match get_config_path() {
        Ok((config_file_path, _)) => {
            println!("Config path is: {}", config_file_path.to_str().unwrap());
        }
        Err(ConfigError(msg)) => {
            eprintln!("Could not look up config_path: {}", msg);
            process::exit(1);
        }
        Err(_) => {
            eprintln!("Unknown error");
            process::exit(1);
        }
    }
}

/// Show file paths
fn show_paths(custom_pages_dir: impl AsRef<Path>) {
    let config_dir = get_config_dir().map_or_else(
        |e| format!("[Error: {}]", e),
        |(mut path, source)| {
            path.push(""); // Trailing path separator
            match path.to_str() {
                Some(path) => format!("{} ({})", path, source),
                None => "[Invalid]".to_string(),
            }
        },
    );
    let config_path = get_config_path().map_or_else(
        |e| format!("[Error: {}]", e),
        |(path, _)| path.to_str().unwrap_or("[Invalid]").to_string(),
    );
    let cache_dir = Cache::get_cache_dir().map_or_else(
        |e| format!("[Error: {}]", e),
        |(mut path, source)| {
            path.push(""); // Trailing path separator
            match path.to_str() {
                Some(path) => format!("{} ({})", path, source),
                None => "[Invalid]".to_string(),
            }
        },
    );
    let pages_dir = Cache::get_cache_dir().map_or_else(
        |e| format!("[Error: {}]", e),
        |(mut path, _)| {
            path.push("tldr-master");
            path.push(""); // Trailing path separator
            path.into_os_string()
                .into_string()
                .unwrap_or_else(|_| String::from("[Invalid]"))
        },
    );
    let custom_pages_dir = custom_pages_dir.as_ref().to_str().unwrap_or("[Invalid]");

    println!("Config dir:       {}", config_dir);
    println!("Config path:      {}", config_path);
    println!("Cache dir:        {}", cache_dir);
    println!("Pages dir:        {}", pages_dir);
    println!("Custom pages dir: {}", custom_pages_dir);
}

/// Create seed config file and exit
fn create_config_and_exit() {
    match make_default_config() {
        Ok(config_file_path) => {
            eprintln!(
                "Successfully created seed config file here: {}",
                config_file_path.to_str().unwrap()
            );
            process::exit(0);
        }
        Err(ConfigError(msg)) => {
            eprintln!("Could not create seed config: {}", msg);
            process::exit(1);
        }
        Err(_) => {
            eprintln!("Unknown error");
            process::exit(1);
        }
    }
}

#[cfg(feature = "logging")]
fn init_log() {
    env_logger::init();
}

#[cfg(not(feature = "logging"))]
fn init_log() {}

#[cfg(target_os = "linux")]
fn get_os() -> OsType {
    OsType::Linux
}

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
fn get_os() -> OsType {
    OsType::OsX
}

#[cfg(target_os = "windows")]
fn get_os() -> OsType {
    OsType::Windows
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
    target_os = "windows"
)))]
fn get_os() -> OsType {
    OsType::Other
}

fn get_languages(env_lang: Option<&str>, env_language: Option<&str>) -> Vec<String> {
    // Language list according to
    // https://github.com/tldr-pages/tldr/blob/master/CLIENT-SPECIFICATION.md#language

    if env_lang.is_none() {
        return vec!["en".to_string()];
    }
    let env_lang = env_lang.unwrap();

    // Create an iterator that contains $LANGUAGE (':' separated list) followed by $LANG (single language)
    let locales = env_language
        .unwrap_or("")
        .split(':')
        .chain(iter::once(env_lang));

    let mut lang_list = Vec::new();
    for locale in locales {
        // Language plus country code (e.g. `en_US`)
        if locale.len() >= 5 && locale.chars().nth(2) == Some('_') {
            lang_list.push(&locale[..5]);
        }
        // Language code only (e.g. `en`)
        if locale.len() >= 2 && locale != "POSIX" {
            lang_list.push(&locale[..2]);
        }
    }

    lang_list.push("en");
    lang_list.clear_duplicates();
    lang_list.into_iter().map(str::to_string).collect()
}

fn get_languages_from_env() -> Vec<String> {
    get_languages(
        std::env::var("LANG").ok().as_deref(),
        std::env::var("LANGUAGE").ok().as_deref(),
    )
}

fn main() {
    // Initialize logger
    init_log();

    // Parse arguments
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    // Show version and exit
    if args.flag_version {
        let os = get_os();
        println!("{} v{} ({})", NAME, VERSION, os);
        process::exit(0);
    }

    // Create a basic config and exit
    if args.flag_seed_config {
        create_config_and_exit();
    }

    // Determine the usage of styles
    #[cfg(target_os = "windows")]
    let ansi_support = ansi_term::enable_ansi_support().is_ok();
    #[cfg(not(target_os = "windows"))]
    let ansi_support = true;

    let enable_styles = match args.flag_color {
        // Attempt to use styling if instructed
        ColorOptions::Always => true,
        // Enable styling if:
        // * There is `ansi_support`
        // * NO_COLOR env var isn't set: https://no-color.org/
        // * The output stream is stdout (not being piped)
        ColorOptions::Auto => {
            ansi_support && env::var_os("NO_COLOR").is_none() && atty::is(Stream::Stdout)
        }
        // Disable styling
        ColorOptions::Never => false,
    };

    // Look up config file, if none is found fall back to default config.
    let config = match Config::load(enable_styles) {
        Ok(config) => config,
        Err(ConfigError(msg)) => {
            eprintln!("Could not load config: {}", msg);
            process::exit(1);
        }
        Err(e) => {
            eprintln!("Could not load config: {}", e);
            process::exit(1);
        }
    };

    // Show config file and path, pass through
    if args.flag_config_path {
        eprintln!("Warning: The --config-path flag is deprecated, use --show-paths instead");
        show_config_path();
    }
    if args.flag_show_paths {
        show_paths(&config.directories.custom_pages_dir);
    }

    if args.flag_pager || config.display.use_pager {
        configure_pager();
    }

    // Specify target OS
    let os: OsType = match args.flag_os {
        Some(os) => os,
        None => get_os(),
    };

    // Initialize cache
    let cache = Cache::new(ARCHIVE_URL, os);

    // Clear cache, pass through
    if args.flag_clear_cache {
        clear_cache(args.flag_quiet);
    }

    // Update cache, pass through
    let cache_updated = if should_update_cache(&args, &config) {
        update_cache(&cache, args.flag_quiet);
        true
    } else {
        false
    };

    // Render local file and exit
    if let Some(ref file) = args.flag_render {
        let path = PageLookupResult::with_page(PathBuf::from(file));
        if let Err(msg) = print_page(&path, args.flag_markdown, &config) {
            eprintln!("{}", msg);
            process::exit(1);
        } else {
            process::exit(0);
        };
    }

    // List cached commands and exit
    if args.flag_list {
        if !cache_updated {
            // Check cache for freshness
            check_cache(&args, enable_styles);
        }

        // Get list of pages
        let pages = cache.list_pages().unwrap_or_else(|e| {
            eprintln!("Could not get list of pages: {}", e.message());
            process::exit(1);
        });

        // Print pages
        println!("{}", pages.join("\n"));
        process::exit(0);
    }

    // Show command from cache
    if let Some(ref command) = args.arg_command {
        let command = command.join("-");

        if !cache_updated {
            // Check cache for freshness
            check_cache(&args, enable_styles);
        }

        let languages = args
            .flag_language
            .map_or_else(get_languages_from_env, |flag_lang| vec![flag_lang]);

        // Search for command in cache
        if let Some(page) =
            cache.find_page(&command, &languages, &config.directories.custom_pages_dir)
        {
            if let Err(msg) = print_page(&page, args.flag_markdown, &config) {
                eprintln!("{}", msg);
                process::exit(1);
            }
            process::exit(0);
        } else {
            if !args.flag_quiet {
                eprintln!("Page {} not found in cache", &command);
                eprintln!("Try updating with `tldr --update`, or submit a pull request to:");
                eprintln!("https://github.com/tldr-pages/tldr");
            }
            process::exit(1);
        }
    }

    // Some flags can be run without a command.
    if !(args.flag_update || args.flag_clear_cache || args.flag_config_path || args.flag_show_paths)
    {
        eprintln!("{}", USAGE);
        process::exit(1);
    }
}

#[cfg(test)]
mod test {
    use crate::{get_languages, Args, OsType, USAGE};
    use docopt::{Docopt, Error};

    fn test_helper(argv: &[&str]) -> Result<Args, Error> {
        Docopt::new(USAGE).and_then(|d| d.argv(argv.iter()).deserialize())
    }

    #[test]
    fn test_docopt_os_case_insensitive() {
        let argv = vec!["cp", "--os", "LiNuX"];
        let os = test_helper(&argv).unwrap().flag_os.unwrap();
        assert_eq!(OsType::Linux, os);
    }

    #[test]
    fn test_docopt_expect_error() {
        let argv = vec!["cp", "--os", "lindows"];
        assert!(!test_helper(&argv).is_ok());
    }

    mod language {
        use super::*;

        #[test]
        fn missing_lang_env() {
            let lang_list = get_languages(None, Some("de:fr"));
            assert_eq!(lang_list, vec!["en"]);
            let lang_list = get_languages(None, None);
            assert_eq!(lang_list, vec!["en"]);
        }

        #[test]
        fn missing_language_env() {
            let lang_list = get_languages(Some("de"), None);
            assert_eq!(lang_list, vec!["de", "en"]);
        }

        #[test]
        fn preference_order() {
            let lang_list = get_languages(Some("de"), Some("fr:cn"));
            assert_eq!(lang_list, vec!["fr", "cn", "de", "en"]);
        }

        #[test]
        fn country_code_expansion() {
            let lang_list = get_languages(Some("pt_BR"), None);
            assert_eq!(lang_list, vec!["pt_BR", "pt", "en"]);
        }

        #[test]
        fn ignore_posix_and_c() {
            let lang_list = get_languages(Some("POSIX"), None);
            assert_eq!(lang_list, vec!["en"]);
            let lang_list = get_languages(Some("C"), None);
            assert_eq!(lang_list, vec!["en"]);
        }

        #[test]
        fn no_duplicates() {
            let lang_list = get_languages(Some("de"), Some("fr:de:cn:de"));
            assert_eq!(lang_list, vec!["fr", "de", "cn", "en"]);
        }
    }
}
