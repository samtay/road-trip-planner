use anyhow::{bail, ensure, Result};
use clap::{App, ArgMatches};
use geo::prelude::*;
use geo::{point, Point};
use itertools::Itertools;
use serde::Deserialize;
use std::process::Command;

fn main() -> Result<()> {
    let cli = cli_opts();
    match cli.subcommand_name() {
        Some("fetch-data") => {
            fetch_data()?;
        }
        Some(_) => {
            bail!("subcommand not recognized");
        }
        None => {
            fetch_data()?;
        }
    }
    Ok(())
}

/// CLI interface
fn cli_opts() -> ArgMatches {
    App::new("road-trip-planner")
        .about("Utility to run the road trip planner")
        .subcommand(App::new("fetch-data").about("Generate data/*.facts"))
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
    let status = Command::new("sh").arg("bin/fetch_nps_data").status()?;
    ensure!(status.success(), "couldn't fetch nps data");
    let status = Command::new("sh").arg("bin/json_to_facts").status()?;
    ensure!(status.success(), "couldn't convert json to facts");
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
        return meters * MILES_PER_METER;
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
