mod crepe;

use anyhow::{bail, ensure, Result};
use clap::{App, ArgMatches};
use geo::prelude::*;
use geo::{point, Point};
use itertools::Itertools;
use serde::Deserialize;
use std::process::{Command, Stdio};

fn main() -> Result<()> {
    let cli = cli_opts();
    if cli.is_present("refresh") {
        fetch_data()?;
    }
    match cli.subcommand() {
        Some(("souffle", _)) => {
            souffle()?;
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
        .subcommand(App::new("souffle").about("Run souffle planner"))
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
        .into_deserialize()
        .filter_map(|x: std::result::Result<LocationRow, csv::Error>| x.ok())
        .combinations(2);
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
    park_name_from: String,
    camp_name_from: String,
    park_name_to: String,
    camp_name_to: String,
    distance: f64,
    stop_ix: u32,
}

/// Run souffle/plan.dl
fn souffle() -> Result<()> {
    let status = Command::new("souffle")
        .arg("--fact-dir")
        .arg("data")
        .arg("--output-dir")
        .arg("output")
        .arg("souffle/plan.dl")
        .status()?;
    ensure!(status.success(), "failed to run souffle");

    let mut stops = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(b'\t')
        .from_path("output/souffle-plan.tsv")?
        .into_deserialize()
        .collect::<Result<Vec<ParkStop>, _>>()?;
    stops.sort_unstable_by(|a, b| a.stop_ix.cmp(&b.stop_ix));

    let name_width = stops
        .iter()
        .map(|s| s.park_name_from.len() + s.camp_name_from.len() + 2)
        .max()
        .unwrap_or(20);

    println!(
        "{name:^width$}",
        name = format!("{}: {}", stops[0].park_name_from, stops[0].camp_name_from),
        width = name_width,
    );
    for stop in stops {
        println!("{:^width$}", "\u{21A1}", width = name_width);
        println!(
            "{name:^width$}",
            name = format!("{}: {}", stop.park_name_to, stop.camp_name_to),
            width = name_width,
        );
    }
    Ok(())
}
