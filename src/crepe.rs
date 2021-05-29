use std::path::Path;

use anyhow::Result;
use crepe::crepe;
use csv::StringRecord;

crepe! {
    @input
    struct Park<'a>(&'a str, &'a str); // park_id, name

    @input
    struct Camp<'a>(&'a str, &'a str, &'a str); // camp_id, park_id, name

    @input
    struct Distance<'a>(&'a str, &'a str, u32); // camp_id, camp_id, distance

    @input
    struct Loc<'a>(&'a str, i64, i64); // camp_id, lat, long

    @input
    struct Amenities<'a>(&'a str, u32); // camp_id, rv_length, ...

    struct RVCamp<'a>(&'a str); // camp_id
    RVCamp(c) <- Camp(c, _, _), Amenities(c, d), (d == 0 || d >= 34);

    // RV distance only goes between campgrounds that fit our RV
    // Also includes directions both ways between campgrounds
    struct RVDistance<'a>(&'a str, &'a str, u32); // camp_id, camp_id, distance
    RVDistance(from, to, len) <- Distance(from, to, len), RVCamp(from), RVCamp(to);
    RVDistance(to, from, len) <- Distance(from, to, len), RVCamp(from), RVCamp(to);

    // A segment is between (100mi, 500mi) and makes northwestern progress
    struct Segment<'a>(&'a str, &'a str, u32);
    Segment(from, to, len) <-
        RVDistance(from, to, len),
        (200 <= len),
        (len <= 600),
        Loc(from, f_lat, f_long),
        Loc(to, t_lat, t_long),
        (t_lat - f_lat > 0 || f_long - t_long > 0);

    // Start in the Everglades
    @output
    struct RoadTrip<'a>(&'a str, &'a str, u32);
    RoadTrip("start", camp_fl, 0) <- Camp(camp_fl, "ever", "Flamingo Campground");
    RoadTrip(from, to, acc + len) <-
      RoadTrip(_, from, acc),
      !Camp(from, "olym", _), // Stop once we get to Olympic National Forest in WA
      Segment(from, to, len);
}

pub fn run() -> Result<Vec<(String, String, u32)>> {
    let parks = fetch_from("data/park.facts")?;
    let camps = fetch_from("data/campground.facts")?;
    let dists = fetch_from("data/distance.facts")?;
    let amenities = fetch_from("data/amenities.facts")?;
    let locs = fetch_from("data/location.facts")?;
    let mut runtime = Crepe::new();
    runtime.extend(parks.iter().map(|rec| Park(&rec[0], &rec[1])));
    runtime.extend(camps.iter().map(|rec| Camp(&rec[0], &rec[1], &rec[2])));
    runtime.extend(dists.iter().map(|rec| {
        Distance(
            &rec[0],
            &rec[1],
            rec[2]
                .parse::<f64>()
                .expect("couldn't convert string to float") as u32,
        )
    }));
    runtime.extend(amenities.iter().map(|rec| {
        Amenities(
            &rec[0],
            rec[1].parse().expect("couldn't convert string to int"),
        )
    }));
    // Floats aren't hashable, so to get more precision in lat/long we need to
    // push the decimal point to the right a few places.
    runtime.extend(locs.iter().map(|rec| {
        Loc(
            &rec[0],
            (rec[1]
                .parse::<f64>()
                .expect("couldn't convert string to float")
                * 1000f64) as i64,
            (rec[2]
                .parse::<f64>()
                .expect("couldn't convert string to float")
                * 1000f64) as i64,
        )
    }));
    let (stops,) = runtime.run();
    let stops = stops
        .into_iter()
        .map(|RoadTrip(a, b, d)| (a.to_owned(), b.to_owned(), d))
        .collect();
    Ok(stops)
}

fn fetch_from(p: impl AsRef<Path>) -> Result<Vec<StringRecord>> {
    let facts = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(b'\t')
        .from_path(p)?
        .into_records()
        .collect::<Result<Vec<_>, _>>()?;
    Ok(facts)
}
