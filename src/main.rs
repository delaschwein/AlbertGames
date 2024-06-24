extern crate regex;

use indexmap::IndexMap;
use indicatif::{ProgressBar, ProgressStyle};
use log::info;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

const GAMES_DIR: &str = "./games";
const RESULTS_DIR: &str = "./results";

#[derive(Serialize, Deserialize, Clone)]
struct Message {
    sender: String,
    recipient: String,
    message: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct Phase {
    messages: Vec<Message>,
    units: IndexMap<String, Vec<String>>,
    scs: IndexMap<String, Vec<String>>,
}

#[derive(Serialize, Deserialize)]
struct Game {
    phases: IndexMap<String, Phase>,
    moves: Vec<String>,
    status: String,
    summary: String,
}

fn filter_lines(lines: Vec<String>, filter_keywords: &[&str]) -> Vec<String> {
    lines
        .into_iter()
        .filter(|line| {
            !filter_keywords
                .iter()
                .any(|&keyword| line.contains(keyword))
        })
        .collect()
}

// Function to write filtered lines to a new file in the results directory
fn write_filtered_lines(filename: &str, game: Game) -> io::Result<()> {
    let results_path: std::path::PathBuf =
        Path::new(RESULTS_DIR).join(filename).with_extension("json");
    info!("Writing to {:?}", results_path);
    let mut file: fs::File = fs::File::create(results_path)?;

    let json = serde_json::to_string_pretty(&game).expect("Failed to serialize to JSON");

    file.write_all(json.as_bytes())?;
    Ok(())
}

// Function to remove entries preceding the first occurrence of "HLO"
fn remove_preceding_entries(lines: Vec<String>) -> Vec<String> {
    if let Some(pos) = lines.iter().position(|line: &String| line.contains("HLO")) {
        lines[pos..].to_vec()
    } else {
        vec![]
    }
}

fn construct_game(input: Vec<String>, power_map: &IndexMap<String, String>) -> Game {
    // group string based on rules
    let mut i = 0;
    let mut phases: IndexMap<String, Phase> = IndexMap::new();
    let mut curr_sc_dist: IndexMap<String, Vec<String>> = IndexMap::new();
    let mut curr_unit_dist: IndexMap<String, Vec<String>> = IndexMap::new();
    let mut moves: Vec<String> = Vec::new();
    let mut curr_phase: String = "".to_string();
    let mut phase_now: Phase = Phase {
        messages: Vec::new(),
        units: IndexMap::new(),
        scs: IndexMap::new(),
    };
    let sc_dist_re: Regex = Regex::new(r"\(\s?[\s?[A-Z]+\s?]+\s?\)").unwrap();
    let mut end_game_status: String = "".to_string();
    let mut summary: String = "".to_string();

    while i < input.len() {
        let line: &str = input[i].trim();
        let mut idx: i32 = -1;
        let is_boardcast: bool = line.starts_with("ALL");
        let mut to_server: bool = false;
        let first_char: char;
        let mut power: &str = "";

        if !is_boardcast {
            first_char = line.chars().next().unwrap();
            power = power_map.get(&first_char.to_string()).unwrap();
        }

        // check if the log is from client to server or server to client
        // might be useful later
        match line.find(">>") {
            // client to server
            Some(i) => {
                idx = i as i32;
                to_server = true;
            }
            None => {}
        }

        match line.find("<<") {
            // server to client
            Some(i) => {
                idx = i as i32;
            }
            None => {}
        }

        assert!(idx != -1, "No >> or << found in line: {}", line);

        // get the substring after the >> or <<
        let line: String = line.chars().skip(idx as usize + 2).collect();
        // trim the line
        let line: &str = line.trim();

        if is_boardcast {
            i += 1;

            if line.contains("ORD") {
                // add to phase_now.moves
                moves.push(line.to_string());
            } else {
                if line.contains("SCO") {
                    let sc_dist: Vec<String> = sc_dist_re
                        .find_iter(line)
                        .map(|m| m.as_str().to_string())
                        .collect();
                    // map to remove the starting and ending brackets
                    let sc_dist: Vec<String> = sc_dist
                        .iter()
                        .map(|s: &String| s.chars().skip(1).take(s.len() - 2).collect())
                        .collect();
                    // trim the strings
                    let sc_dist: Vec<String> = sc_dist
                        .iter()
                        .map(|s: &String| s.trim().to_string())
                        .collect();
                    // iterate over sc_dist
                    for sc in sc_dist.iter() {
                        // split the string by spaces
                        let mut sc: Vec<&str> = sc.split_whitespace().collect();

                        // remove the first element
                        let power: &str = sc.remove(0);

                        // convert to vector of strings
                        let sc: Vec<String> = sc.iter().map(|s: &&str| s.to_string()).collect();
                        curr_sc_dist.insert(power.to_string(), sc);
                    }
                } else if line.contains("NOW") {
                    if curr_phase != "" {
                        phases.insert(curr_phase.clone(), phase_now.clone());
                    }

                    // substring(3, end)
                    let line: String = line.chars().skip(3).collect();
                    let line: &str = line.trim();
                    assert!(
                        !line.contains("NOW"),
                        "NOW should not be in the line: {}",
                        line
                    );
                    // iterate over line by char
                    let mut curr_substring: Vec<char> = Vec::new();

                    for ii in 0..line.len() {
                        let c: char = line.chars().nth(ii).unwrap();
                        if c == '(' {
                            // append to curr_substring until the closing bracket
                            let mut j = ii + 1;
                            while j < line.len() {
                                let c: char = line.chars().nth(j).unwrap();
                                if c == ')' {
                                    let s: String = curr_substring.iter().collect();
                                    let s: &str = s.trim();

                                    if s.contains("19") {
                                        curr_phase = s.to_string();
                                    } else {
                                        assert!(s.len() > 7, "Invalid unit distribution: {}", s);
                                        let power: &str = &s[0..3];
                                        let unit_type: &str = &s[4..7];
                                        let unit: &str = &s[7..];

                                        // check if the power exists in unit_dist
                                        if !curr_unit_dist.contains_key(power) {
                                            curr_unit_dist.insert(power.to_string(), Vec::new());
                                        }

                                        if let Some(units) = curr_unit_dist.get_mut(power) {
                                            let unit_str: String =
                                                unit_type.chars().next().unwrap().to_string()
                                                    + " "
                                                    + unit;
                                            units.push(unit_str);
                                        }
                                    }
                                    break;
                                }
                                curr_substring.push(c);
                                j += 1;
                            }
                        }
                    }
                    // update phase_now using curr_unit_dist and curr_sc_dist
                    phase_now.units = curr_unit_dist.clone();
                    phase_now.scs = curr_sc_dist.clone();
                } else if line.contains("SLO") || line.contains("DRW") {
                    end_game_status = line.to_string();
                } else if line.contains("SMR") {
                    summary = line.to_string();
                }
            }
        } else {
            i += 1;

            if to_server && line.starts_with("SND") {
                assert!(line.len() > 25, "Invalid SND line: {}", line);
                let to_power: &str = &line[19..22];
                let content: &str = &line[25..];
                let message: Message = Message {
                    sender: power.to_string(),
                    recipient: to_power.to_string(),
                    message: content.to_string(),
                };
                phase_now.messages.push(message);
            }
        }
    }
    let final_game: Game = Game {
        phases: phases,
        status: end_game_status,
        summary: summary,
        moves: moves,
    };
    final_game
}

fn main() -> io::Result<()> {
    // Create the results directory if it doesn't exist
    fs::create_dir_all(RESULTS_DIR)?;

    info!("Reading files from {}", GAMES_DIR);
    // Remove all files in the results directory
    for entry in fs::read_dir(RESULTS_DIR)? {
        let entry: fs::DirEntry = entry?;
        if entry.path().is_file() {
            fs::remove_file(entry.path())?;
        }
    }

    let paths: fs::ReadDir = fs::read_dir(GAMES_DIR)?;

    let paths_count: usize = paths.count();
    let bar = ProgressBar::new(paths_count as u64);
    bar.set_style(ProgressStyle::default_bar());

    let paths: fs::ReadDir = fs::read_dir(GAMES_DIR)?;

    // Filter out unnecessary lines
    // - "==": server logs such as message syntax check
    // - "ADM": Admin messages
    // - "NME": Client to server names
    // - "MDF": Map definition
    // - "8 >> | <<": Observer
    // - "GOF": whether to process orders
    let filter_keywords: [&str; 7] = ["==", "ADM", "NME", "MDF", "8 >>", "8 <<", "GOF"];

    for path in paths {
        let path: std::path::PathBuf = path?.path();
        if path.is_file() {
            let file_name: &str = path.file_name().and_then(OsStr::to_str).unwrap_or("");
            let file: fs::File = fs::File::open(&path)?;
            let reader: io::BufReader<fs::File> = io::BufReader::new(file);
            let mut lines: Vec<String> = Vec::new();

            for line in reader.lines() {
                if let Ok(line) = line {
                    lines.push(line);
                }
            }

            // trim all lines
            lines
                .iter_mut()
                .for_each(|line: &mut String| *line = line.trim().to_string());

            // Filter lines using the filter_lines function
            let filtered_lines: Vec<String> = filter_lines(lines, &filter_keywords);

            // Remove entries preceding the first occurrence of "HLO"
            let mut final_lines: Vec<String> = remove_preceding_entries(filtered_lines);

            // remove the first 7 lines. Assume they all contain "HLO"
            let mut power_map: IndexMap<String, String> = IndexMap::new();

            // hashmap to store power ids
            for _ in 0..7 {
                let ll: String = final_lines.remove(0);
                let trimmed: &str = ll.trim();
                let first_char: char = trimmed.chars().next().unwrap();

                let re: Regex = Regex::new(r"\(\s*([A-Z]{3})\s*\)").unwrap();
                let country_code: &str = re
                    .captures(trimmed)
                    .and_then(|caps| caps.get(1))
                    .map_or("", |m| m.as_str());

                power_map.insert(first_char.to_string(), country_code.to_string());
            }

            // group the strings
            //let grouped_strings: Vec<Vec<String>> = group_strings(final_lines, &power_map);

            let game: Game = construct_game(final_lines, &power_map);

            write_filtered_lines(file_name, game)?;
        }
        bar.inc(1);
    }

    bar.finish();
    Ok(())
}
