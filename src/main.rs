use anyhow::Result;
use clap::{App, ArgMatches};
use geo::prelude::HaversineDistance;
use geo::{point, Point};
use itertools::Itertools;
use serde::Deserialize;

fn main() -> Result<()> {
    let cli = cli_opts();
    // TODO pass sub_command matches to `fetch` and parse this key over there
    match cli.subcommand_name() {
        Some("fetch") => {}
        Some("refresh-distances") => {
            refresh_distances()?;
        }
        _ => (),
    }
    Ok(())
}

/// CLI interface
fn cli_opts() -> ArgMatches {
    App::new("road-trip-planner")
        .about("Utility to run the road trip planner")
        //.arg("-k, --key=[API-KEY] 'Set NPS API key'")
        .subcommand(App::new("refresh-distances").about("Generate distance.facts"))
        .get_matches()
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct LocationRow {
    camp_id: String,
    latitude: f64,
    longitude: f64,
}

impl LocationRow {
    fn coordinate(&self) -> Point<f64> {
        point!(x: self.latitude, y: self.longitude)
    }

    /// Distance to another location in miles
    pub fn distance_to(&self, other: &LocationRow) -> f64 {
        const MILES_PER_METER: f64 = 0.000621371;
        let meters = self.coordinate().haversine_distance(&other.coordinate());
        return meters * MILES_PER_METER;
    }
}

/// Generate distance.facts
fn refresh_distances() -> Result<()> {
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
        //.quote_style(?)
        .from_path("data/distance.facts")?;
    for pair in pairs {
        writer.write_record(&[
            &pair[0].camp_id,
            &pair[1].camp_id,
            &pair[0].distance_to(&pair[1]).round().to_string(),
        ])?;
    }
    Ok(())
}
