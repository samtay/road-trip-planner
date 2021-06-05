use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;

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

/// Since the list just contains each stop instead of (from, to) pairs, this
/// parser can be really simple, and just strip the list brackets
fn parse_stops(plan: &str) -> Result<Vec<&str>> {
    let parse_err = || anyhow!("unexpected output from plan enumeration");
    let mut stops = plan
        // Strip the leading cons list chars
        .split_once("nil, [")
        .ok_or_else(parse_err)?
        .1
        .split("]], [")
        // Only keep the park/camp name
        .map(|s| Ok(s.rsplit_once(',').ok_or_else(parse_err)?.0))
        .collect::<Result<Vec<_>>>()?;
    if let Some(last_stop) = stops.last_mut() {
        *last_stop = last_stop.trim_end_matches(']');
    }
    Ok(stops)
}

fn print_stops(stops: Vec<&str>) -> Result<()> {
    let width = stops.iter().map(|s| s.len()).max().unwrap_or(20);
    let mut first = true;
    for stop in stops {
        if !first {
            println!("{:^width$}", "\u{21A1}", width = width);
        } else {
            first = false;
        }
        println!("{:^width$}", stop, width = width);
    }
    Ok(())
}
