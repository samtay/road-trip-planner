mod crepe;
mod parser;

use anyhow::{anyhow, bail, ensure, Result};
use clap::{App, ArgMatches};
use geo::prelude::*;
use geo::{point, Point};
use itertools::Itertools;
use serde::Deserialize;
use std::io::Write;
use std::process::{Command, ExitStatus, Stdio};

fn main() -> Result<()> {
    let cli = cli_opts();
    if cli.is_present("refresh") {
        fetch_data()?;
    }
    match cli.subcommand() {
        Some(("souffle", m)) => {
            souffle_populate_input_files(m)?;
            if m.is_present("enumerate") {
                souffle_enumerate()?;
            } else {
                souffle_choice(m)?;
            }
        }
        Some(("crepe", _)) => {
            println!("{:?}", crepe::run());
        }
        Some(_) => {
            bail!("subcommand not recognized");
        }
        None => {}
    }
    Ok(())
}

/// CLI interface
fn cli_opts() -> ArgMatches {
    App::new("road-trip-planner")
        .about("Utility to run the road trip planner")
        .arg("-r, --refresh 'Use fresh NPS data'")
        .subcommand(
            App::new("souffle")
                .about("Run souffle planner")
                .arg("-e --enumerate 'Enumerate all trips'")
                .arg("--min 'Use minimum distance between stops'")
                .arg("<from> 'Starting park code (e.g. ever)'")
                .arg("<to> 'Ending park code (e.g. olym)'"),
        )
        .subcommand(App::new("crepe").about("Run crepe planner"))
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
            .stdout(Stdio::null())
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
    distance: f64,
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
    let name_width = stops
        .iter()
        .map(|s| s.park_name.len() + s.camp_name.len() + 2)
        .max()
        .unwrap_or(20);

    ensure!(
        stops.len() > 1,
        "No road trip found for the given constraints"
    );

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

/// Run souffle/plan-enumerate.dl to output all possible plans
/// Warning: Easily runs out of memory for road trips of significant size!
fn souffle_enumerate() -> Result<()> {
    let status = run_souffle_cmd("souffle/plan-enumerate.dl")?;
    ensure!(status.success(), "failed to run souffle");
    parser::parse_enumerate_output()
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
