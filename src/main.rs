use anyhow::Result;
use clap::{App, ArgMatches};
use serde_json::Value;
use std::env;
use std::fs::File;

fn main() -> Result<()> {
    let cli = cli_opts();
    let key = cli
        .value_of("key")
        .map(String::from)
        .or_else(|| env::var("NPS_API_KEY").ok());
    match cli.subcommand_name() {
        Some("fetch") => {
            fetch(key.unwrap())?;
            populate_facts()?;
        }
        _ => (),
    }
    Ok(())
}

/// CLI interface
fn cli_opts() -> ArgMatches {
    App::new("road-trip-planner")
        .about("Utility to run the road trip planner")
        .arg("-k, --key=[API-KEY] 'Set NPS API key'")
        .subcommand(
            App::new("fetch").about("fetch the data"), //.arg("--path 'Path to output file'"),
        )
        .get_matches()
}

/// Fetch data from NPS
fn fetch(key: String) -> Result<()> {
    let json: Value = ureq::get("https://developer.nps.gov/api/v1/campgrounds")
        .set("X-Api-Key", &key)
        .query("limit", "613")
        .call()?
        .into_json()?;
    let file = File::create("data/campgrounds.json")?;
    serde_json::to_writer(file, &json)?;
    Ok(())
}

fn populate_facts() -> Result<()> {
    // Fields of interest:
    // parkCode
    // name
    // latitude
    // longitude // or "latLong": "{lat:29.51187, lng:-100.907479}"
    // accessibility
    //   rvAllowed == "1"
    //   rvMaxLength ? == "0" or >=34
    // amenities
    //   internetConnectivity
    //   cellPhoneReception
    //   dumpStation
    // fees
    Ok(())
}
