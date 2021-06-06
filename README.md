# road-trip-planner

A primitive road trip planner between U.S. National Parks. This project uses
Datalog to discover feasible road trip plans against data from the [National
Park Service](https://www.nps.gov/subjects/developer/api-documentation.htm).
The core logic is in the [souffle](./souffle) directory, with a small 
utility CLI runner on top.

This was my final project for a course in Database Management Systems.
I wrote about the exploration [here](https://samtay.github.io/posts/road-trip-planner), if you are interested.

## reqs

1. [souffle](https://souffle-lang.github.io/install)
2. [cargo](https://www.rust-lang.org/tools/install)

**Note**: To run the planner with the `--lucky` option to output a single plan, you'll
need to build souffle from source from a recent commit, as the [choice
construct](https://souffle-lang.github.io/choice) has not yet been released.

## install

Basically just clone the repo. This tool can be used via `cargo run`, or you can
install it to your PATH e.g.
```shell
# get repo and install binary
git clone git@github.com:samtay/road-trip-planner.git
cd road-trip-planner
cargo install --path .
```
but either way, this tool expects to be run from the root of this repository, so
it can easily access the `data`, `souffle`, and `output` directories.

Currently this planner looks for campgrounds that allow a 34' RV. You can adjust
that variable
[here](https://github.com/samtay/road-trip-planner/blob/ce3b291ff6916b04febeb0c5a961a71ca928c4b9/souffle/plan-enumerate.dl#L40).

## usage

The help output explains the usage:

```
USAGE:
    road-trip-planner [FLAGS] <from> <to>

ARGS:
    <from>    Starting park code (e.g. ever)
    <to>      Ending park code (e.g. olym)

FLAGS:
    -h, --help       Prints help information
    -l, --lucky      Output a single trip
        --min        Use minimum distance between stops (for --lucky)
    -r, --refresh    Use fresh NPS data
    -V, --version    Prints version information
```

But this requires a few explanations: the park codes can be found in the
[park.facts](./data/park.facts) file. Also, enumeration may take a very long
time if the parks are far apart; if this happens, try adjusting the required
progress between segments
[here](https://github.com/samtay/road-trip-planner/blob/ce3b291ff6916b04febeb0c5a961a71ca928c4b9/souffle/plan-enumerate.dl#L56).
The `--lucky` option uses souffle's choice construct to make a single plan;
however, it doesn't do this in any intelligent way, so it is possible for the
plan to fail and stop short before the final destination.

Using `--refresh` will fetch the latest data from NPS, however this does require
a valid `NPS_API_KEY` environment variable. This key can be obtained for free
from NPS [here](https://www.nps.gov/subjects/developer/get-started.htm).

## examples

```shell
# non-deterministically find a single plan from badlands to glacier national park
❯ road-trip-planner --lucky badl glac
       Badlands National Park: Cedar Pass Campground         (0.00)
                             ↡
Bighorn Canyon National Recreation Area: Afterbay Campground (314.16)
                             ↡
                Glacier National Park: Apgar                 (675.59)

# enumerate all plans from yosemite to the olympic national forest
❯ road-trip-planner yose olym
Plan 1
------
    Yosemite National Park: Crane Flat Campground     (0.00)
                          ↡
Whiskeytown National Recreation Area: Brandy Creek RV (247.61)
                          ↡
Olympic National Park: Heart O' the Hills Campground  (761.96)
.
.
.
Plan 4316
---------
            Yosemite National Park: Wawona Campground              (0.00)
                                ↡
     Lassen Volcanic National Park: Warner Valley Campground       (220.53)
                                ↡
Lake Roosevelt National Recreation Area: Spring Canyon Group Sites (752.22)
                                ↡
              Olympic National Park: Mora Campground               (1014.45)

```

### todo

1. Include the cell, internet, and dump amenities in the enumeration output.
   These can then be externally counted, so that plans can be sorted by stops
   with the most cell service, or to filter plans that include at least one dump
   stop every 500mi, etc..
2. A few more things should be parameterized from the CLI, such as the RV
   length, minimum progress and maximum segment distance. The minimum progress
   turns out to be a very important parameter, and makes the difference of
   whether or not the computation finishes in a second or days.
3. Call out to an API for actual driving distance and/or time, instead of using
   the haversine approximation.
4. Indicate an error when the choice domain fails to reach the final
   destination.
