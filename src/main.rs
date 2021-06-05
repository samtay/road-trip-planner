use anyhow::{anyhow, ensure, Result};
use clap::{App, ArgMatches};
use geo::prelude::*;
use geo::{point, Point};
use itertools::Itertools;
use serde::Deserialize;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, Write};
use std::process::{Command, ExitStatus, Stdio};

fn main() -> Result<()> {
    let cli = cli_opts();
    if cli.is_present("refresh") {
        fetch_data()?;
    }
    souffle_populate_input_files(&cli)?;
    if cli.is_present("enumerate") {
        souffle_enumerate()?;
    } else {
        souffle_choice(&cli)?;
    }
    Ok(())
}

/// CLI interface
fn cli_opts() -> ArgMatches {
    App::new("road-trip-planner")
        .author("Sam Tay, samctay@pm.me")
        .version("0.0.1")
        .about("Generates road trip plans via national parks")
        .arg("-r, --refresh 'Use fresh NPS data'")
        .arg("-e --enumerate 'Enumerate all trips'")
        .arg("--min 'Use minimum distance between stops'")
        .arg("<from> 'Starting park code (e.g. ever)'")
        .arg("<to> 'Ending park code (e.g. olym)'")
        .get_matches()
}

/// Generate all the data/*.facts files with refreshed data from NPS
fn fetch_data() -> Result<()> {
    fetch_nps_data()?;
    generate_distances()?;
    Ok(())
}

/// Fetch NPS data (via scripts in ./bin)
fn fetch_nps_data() -> Result<()> {
    fn run_script<S: AsRef<std::ffi::OsStr>>(script_path: S) -> Result<bool> {
        let status = Command::new("sh")
            .arg(script_path)
            .stderr(Stdio::null())
            .status()?;
        Ok(status.success())
    }
    ensure!(run_script("bin/fetch_nps_data")?, "couldn't fetch nps data");
    ensure!(
        run_script("bin/json_to_facts")?,
        "couldn't convert json to facts"
    );
    Ok(())
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct LocationRow {
    camp_id: String,
    latitude: f64,
    longitude: f64,
}

impl LocationRow {
    fn coordinate(&self) -> Point<f64> {
        point!(x: self.longitude, y: self.latitude)
    }

    /// Distance to another location in miles
    pub fn distance_to(&self, other: &LocationRow) -> f64 {
        const MILES_PER_METER: f64 = 0.000621371;
        let meters = self.coordinate().haversine_distance(&other.coordinate());
        meters * MILES_PER_METER
    }
}

/// Generate distance.facts from the NPS location.facts file
fn generate_distances() -> Result<()> {
    let pairs = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(b'\t')
        .from_path("data/location.facts")?
        .into_deserialize::<LocationRow>()
        .filter_map(|x| x.ok())
        .combinations_with_replacement(2);
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .delimiter(b'\t')
        .from_path("data/distance.facts")?;
    for pair in pairs {
        writer.write_record(&[
            &pair[0].camp_id,
            &pair[1].camp_id,
            &format!("{:.2}", &pair[0].distance_to(&pair[1])),
        ])?;
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct ParkStop {
    park_name: String,
    camp_name: String,
    acc_distance: f64,
    stop_ix: u32,
}

/// Run souffle/plan-choice.dl to output a single plan
fn souffle_choice(cli: &ArgMatches) -> Result<()> {
    std::fs::remove_file("output/souffle-plan-choice.tsv").unwrap_or(());
    std::fs::remove_file("output/souffle-plan-choice-min.tsv").unwrap_or(());
    let fp = if cli.is_present("min") {
        "output/souffle-plan-choice-min.tsv"
    } else {
        "output/souffle-plan-choice.tsv"
    };
    let status = run_souffle_cmd("souffle/plan-choice.dl")?;
    ensure!(status.success(), "failed to run souffle");
    let mut stops = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(b'\t')
        .from_path(fp)?
        .into_deserialize()
        .collect::<Result<Vec<ParkStop>, _>>()?;
    stops.sort_unstable_by(|a, b| a.stop_ix.cmp(&b.stop_ix));
    ensure!(
        stops.len() > 1,
        "No road trip found for the given constraints. Try using --enumerate"
    );
    print_stops(stops)
}

/// Run souffle/plan-enumerate.dl to output all possible plans
/// Warning: Easily runs out of memory for road trips of significant size!
fn souffle_enumerate() -> Result<()> {
    let status = run_souffle_cmd("souffle/plan-enumerate.dl")?;
    ensure!(status.success(), "failed to run souffle");
    parse_enumerate_output()
}

/// Read the souffle output cons list and spit it out in a readable format to stdout
/// TODO take bufreader and bufwriter
pub fn parse_enumerate_output() -> Result<()> {
    let file = File::open("output/souffle-plan-enumerate.tsv")?;
    let reader = BufReader::new(file);

    let mut count = 0;
    for plan in reader.lines() {
        count += 1;
        let plan = plan?;
        let stops = parse_stops(&plan)?;
        let plan_ix_str = format!("Plan {}", count);
        println!("{}", plan_ix_str);
        println!("{:-^width$}", "", width = plan_ix_str.len());
        print_stops(stops)?;
        println!("{:=^90}", "");
    }
    Ok(())
}

/// Each list item looks like [camp_id, park_name, camp_name, acc_distance, stop_ix]
fn parse_stops(plan: &str) -> Result<Vec<ParkStop>> {
    let parse_err = || anyhow!("unexpected output from plan enumeration");
    let mut stops = plan
        // Strip the leading cons list chars
        .split_once("nil, [")
        .ok_or_else(parse_err)?
        .1
        .split("]], [")
        .collect::<Vec<_>>();
    if let Some(last_stop) = stops.last_mut() {
        *last_stop = last_stop.trim_end_matches(']');
    }
    let stops = stops
        .into_iter()
        .map(|s| {
            let parts = s.split(", ").collect::<Vec<_>>();
            ParkStop {
                park_name: String::from(parts[1]),
                camp_name: String::from(parts[2]),
                acc_distance: parts[3].parse().expect("couldn't convert string to float"),
                stop_ix: parts[4].parse().expect("couldn't convert string to float"),
            }
        })
        .collect::<Vec<_>>();
    Ok(stops)
}

fn print_stops(stops: Vec<ParkStop>) -> Result<()> {
    let name_width = stops
        .iter()
        .map(|s| s.park_name.len() + s.camp_name.len() + 2)
        .max()
        .unwrap_or(20);

    let mut first = true;
    for stop in stops {
        if !first {
            println!("{:^width$}", "\u{21A1}", width = name_width);
        } else {
            first = false;
        }
        let name = format!(
            "{name:^width$}",
            name = format!("{}: {}", stop.park_name, stop.camp_name),
            width = name_width,
        );
        let dist = format!("{stop:>4.2}", stop = stop.acc_distance);
        println!("{} ({})", name, dist);
    }
    Ok(())
}

fn run_souffle_cmd<S: AsRef<std::ffi::OsStr>>(filename: S) -> Result<ExitStatus> {
    let status = Command::new("souffle")
        //.stderr(Stdio::null())
        .arg("--no-warn")
        .arg("--fact-dir")
        .arg("data")
        .arg("--output-dir")
        .arg("output")
        .arg(filename)
        .status()?;
    Ok(status)
}

fn souffle_populate_input_files(cli: &ArgMatches) -> Result<()> {
    let mut start_writer = std::fs::File::create("data/from_park")?;
    cli.value_of("from")
        .ok_or_else(|| anyhow!("FROM argument is required"))
        .and_then(|from| {
            write!(&mut start_writer, "{}", from)?;
            Ok(())
        })?;
    let mut end_writer = std::fs::File::create("data/to_park")?;
    cli.value_of("to")
        .ok_or_else(|| anyhow!("TO argument is required"))
        .and_then(|to| {
            write!(&mut end_writer, "{}", to)?;
            Ok(())
        })?;
    Ok(())
}
