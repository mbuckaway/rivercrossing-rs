// SPDX-License-Identifier: GPL-3.0-only

//! The Rider Simulator's decisions and its one scripted race.
//!
//! The dialog generates placeholder entries and riders from the roster's own
//! primitives, then replays one fixed crossing order lap after lap through a
//! real ride engine. The order holds one plate per entry on either plate model:
//! a relay team crosses once a lap under the entry's own plate, and a pooled
//! team crosses once a lap under the plate of the one rider on course that lap,
//! so a team's lap count equals a solo's. One seed reproduces a whole run.
//!
//! Three configurable behaviours bend that script so a demo shows the
//! console's review surfaces: short-lap riders cross impossibly fast, lapped
//! riders miss the first wave, and team riders stop after the fourth lap. Each
//! count selects the leading positions of the run's one shuffled order.
//!
//! The rules follow the Python oracle's `ui/presenters/simulator.py`. The
//! oracle seeds `CPython`'s Mersenne Twister; the port keeps a local 64-bit
//! generator, so one seed still reproduces a run without a new dependency.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the EPIC-12d views wire this simulator in a later phase; phase 1 unit-tests it"
    )
)]

use std::collections::{HashMap, HashSet};

use rivercrossing_ride::{
    Clock, EntryId, PlateModel, RideEngine, RideError, SystemClock, Timestamp,
};
use rivercrossing_roster::enums::EntryType;
use rivercrossing_roster::roster::{MIN_TEAM_SIZE, Roster, TeamFields};
use rivercrossing_roster::types::{Entry, Rider};
use time::Duration;

#[cfg(test)]
mod tests;

/// The smallest legal team size (R-12), as the roster crate defines it.
const SMALLEST_TEAM: u32 = MIN_TEAM_SIZE;

/// The roster's own default largest team (the setup dialog's 4).
pub(crate) const DEFAULT_MAX_TEAM_SIZE: i64 = 4;

/// The simulation dialog's smallest interval, in minutes.
pub(crate) const INTERVAL_MIN_MINUTES: i64 = 1;

/// The simulation dialog's largest interval, in minutes.
pub(crate) const INTERVAL_MAX_MINUTES: i64 = 240;

/// The buffer above one lap at the average speed.
const INTERVAL_BUFFER_MINUTES: i64 = 5;

/// How far under the ride's minimum a simulated short lap sits.
const SHORT_LAP_MARGIN_S: i64 = 30;

/// The zero-based lap after which a stopped team rider never crosses again.
const TEAM_STOP_LAPS: usize = 4;

/// The seconds in one minute.
const SECONDS_PER_MINUTE: i64 = 60;

/// The minutes in one hour, for the lap-time formula.
const MINUTES_PER_HOUR: f64 = 60.0;

/// The two sexes a generated rider is given.
const SEXES: [&str; 2] = ["M", "F"];

/// The generator's fixed increment.
const LCG_INCREMENT: u64 = 1_442_695_040_888_963_407;

/// The generator's fixed multiplier.
const LCG_MULTIPLIER: u64 = 6_364_136_223_846_793_005;

/// The dialog's generator inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GeneratorInput {
    /// The total number of riders to generate.
    pub(crate) total: i64,
    /// The number of teams to create when the roster holds none.
    pub(crate) teams: i64,
    /// How many of `total` ride solo.
    pub(crate) solo: i64,
    /// The generator's seed.
    pub(crate) seed: u64,
}

/// One simulation's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SimulationSettings {
    /// How many laps to simulate.
    pub(crate) laps: i64,
    /// The minutes between one lap and the next.
    pub(crate) interval_minutes: i64,
    /// How many of the run's leading entries cross under the minimum lap.
    pub(crate) short_laps: i64,
    /// How many of them miss the first wave.
    pub(crate) lapped: i64,
    /// How many team riders stop after the fourth lap.
    pub(crate) team_stop: i64,
    /// The seed the crossing-order shuffle reads.
    pub(crate) seed: u64,
}

/// The result of one simulated race.
///
/// `blocked` holds one reason per issue that stopped the run: the engine's own
/// start refusal, or a refused stop at the end. `recorded` counts only the
/// crossings the engine accepted; `cancelled` is true when the caller's own
/// cancel check ended the run early.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SimOutcome {
    /// Whether the caller's cancel check ended the run early.
    pub(crate) cancelled: bool,
    /// The crossings the engine accepted.
    pub(crate) recorded: usize,
    /// The reasons the run could not be bookkept, when any.
    pub(crate) blocked: Option<Vec<String>>,
}

/// Return the solo count that `riders` and `teams` imply, else `None`.
///
/// The dialog's auto-fill rule: the team riders must land in
/// `min_team_size * teams .. max_team_size * teams`. Teams fill first, so a
/// field large enough for full teams leaves the remainder solo, a field that
/// fits on teams alone leaves none, and a field below the floor has no valid
/// solo count at all.
#[must_use]
#[expect(
    clippy::too_many_arguments,
    reason = "the dialog's rule takes the two counts and the ride's own team-size bounds"
)]
pub(crate) fn resolve_solo(
    riders: i64,
    teams: i64,
    min_team_size: i64,
    max_team_size: i64,
) -> Option<i64> {
    let lowest = min_team_size.saturating_mul(teams);
    let highest = max_team_size.saturating_mul(teams);
    if riders >= highest {
        return Some(riders.saturating_sub(highest));
    }
    if riders >= lowest {
        return Some(0);
    }
    None
}

/// Return the opening interval between one lap and the next, in minutes.
///
/// One lap at the average speed, rounded to the whole minute, plus a
/// five-minute buffer. The result is clamped into the caller's spin bounds.
#[must_use]
#[expect(
    clippy::too_many_arguments,
    reason = "the formula takes the lap, the speed and the caller's own spin bounds"
)]
pub(crate) fn default_interval_minutes(
    lap_km: f64,
    avg_speed_kmh: f64,
    minimum: i64,
    maximum: i64,
) -> i64 {
    let lower = minimum.min(maximum);
    let upper = maximum.max(minimum);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the interval is a whole count of minutes, clamped into the spin's own bounds"
    )]
    let minutes = (lap_km / avg_speed_kmh * MINUTES_PER_HOUR).round_ties_even() as i64;
    minutes
        .saturating_add(INTERVAL_BUFFER_MINUTES)
        .clamp(lower, upper)
}

/// Return the Check button's riders/teams/solo explanation.
///
/// The message says what the relationship is, names the bounds the current
/// counts imply and, when the counts cannot work, how to fix them.
#[must_use]
#[expect(
    clippy::too_many_arguments,
    reason = "the message spells out the two counts and the ride's own team-size bounds"
)]
pub(crate) fn check_message(
    riders: i64,
    teams: i64,
    min_team_size: i64,
    max_team_size: i64,
) -> String {
    let lowest = min_team_size.saturating_mul(teams);
    let highest = max_team_size.saturating_mul(teams);
    let riders_word = plural(riders, "rider", "riders");
    let teams_word = plural(teams, "team", "teams");
    let head = format!(
        "Team riders = teams × riders per team ({min_team_size}–{max_team_size} per team). \
         Solo riders = riders − team riders."
    );
    let counts = format!(
        "With {riders} {riders_word} and {teams} {teams_word}, \
         team riders must be between {lowest} and {highest}"
    );
    let Some(solo) = resolve_solo(riders, teams, min_team_size, max_team_size) else {
        let mut fixes = vec![format!("Increase Number of riders to at least {lowest}")];
        let reduction = riders
            .checked_div(min_team_size)
            .filter(|_reduced| riders >= min_team_size);
        if let Some(reduced) = reduction {
            fixes.push(format!("or reduce Number of teams to {reduced}"));
        }
        return format!(
            "{head}\n{counts}, but the field has only {riders} {riders_word}.\n{}.",
            fixes.join(" ")
        );
    };
    format!(
        "{head}\n{counts}; this field fills the teams to {}, so solo riders = {solo}. Ready to generate.",
        riders.saturating_sub(solo)
    )
}

/// Return the singular or plural word for `count`.
fn plural<'a>(count: i64, one: &'a str, many: &'a str) -> &'a str {
    if count == 1 { one } else { many }
}

/// Create `count` empty teams named `TEAM-0001` upward.
///
/// # Errors
///
/// Return the roster's refusal when the ride has left draft or the ride is
/// solo-only.
pub(crate) fn generate_teams(roster: &mut Roster, count: i64) -> Result<(), String> {
    for index in 1..=count {
        let display_name = format!("TEAM-{index:04}");
        roster
            .create_empty_team(TeamFields {
                display_name: &display_name,
                riders: Vec::new(),
                plate: None,
                logo_card: None,
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Generate `input.total` riders: `input.solo` soloists, the rest on teams.
///
/// The solo entries come first, so their plates sit below every team rider's.
/// The team riders attach round-robin, so each team ends with two to
/// `max_team_size` riders whenever the caller's bounds hold. Names run one
/// shared counter, so every generated name is unique. Teams are created first
/// when the roster holds none.
///
/// # Errors
///
/// Return the refusal for an input outside the generator's bounds, and the
/// roster's own refusal when a create or an attach fails. Nothing is written
/// when the input is refused.
pub(crate) fn generate_riders(roster: &mut Roster, input: &GeneratorInput) -> Result<(), String> {
    require_generator_inputs(input, i64::from(roster.max_team_size()))?;
    let mut rng = Seeded::new(input.seed);
    if team_entries(roster).is_empty() {
        generate_teams(roster, input.teams)?;
    }
    let soloists = usize::try_from(input.solo).unwrap_or(0);
    for counter in 1..=soloists {
        create_solo(roster, counter, &mut rng)?;
    }
    let pooled = roster.plate_model() == PlateModel::RiderPooled;
    let targets: Vec<EntryId> = team_entries(roster).into_iter().map(Entry::id).collect();
    let team_riders = usize::try_from(input.total.saturating_sub(input.solo)).unwrap_or(0);
    for (offset, entry_id) in targets.iter().cycle().take(team_riders).enumerate() {
        add_team_rider(roster, entry_id, soloists + offset + 1, pooled, &mut rng)?;
    }
    Ok(())
}

/// Generate `total` solo entries for a SOLO-only ride.
///
/// # Errors
///
/// Return the refusal for a total below one, and the roster's own refusal when
/// a create fails. Nothing is written when the total is refused.
pub(crate) fn generate_solo_riders(
    roster: &mut Roster,
    total: i64,
    seed: u64,
) -> Result<(), String> {
    if total < 1 {
        return Err("total must be at least 1".to_owned());
    }
    let mut rng = Seeded::new(seed);
    for counter in 1..=usize::try_from(total).unwrap_or(0) {
        create_solo(roster, counter, &mut rng)?;
    }
    Ok(())
}

/// Return why `laps` and `interval_minutes` cannot run, else `None`.
#[must_use]
pub(crate) fn validate(laps: i64, interval_minutes: i64) -> Option<String> {
    let mut problems = Vec::new();
    if laps < 1 {
        problems.push("laps must be at least 1");
    }
    if interval_minutes < 1 {
        problems.push("interval must be at least 1 minute");
    }
    if problems.is_empty() {
        return None;
    }
    Some(problems.join("; "))
}

/// Return the gap one simulated short lap leaves, in seconds.
///
/// The gap is the ride's own minimum less [`SHORT_LAP_MARGIN_S`], clamped at one
/// second, so the derived lap time lands under the minimum and the engine's own
/// rule flags the crossing.
#[must_use]
pub(crate) fn short_lap_gap(min_lap_s: u32) -> i64 {
    (i64::from(min_lap_s) - SHORT_LAP_MARGIN_S).max(1)
}

/// Replay one scripted race of `settings.laps` laps through `engine`.
///
/// The gun is back-dated so the console's elapsed clock reads the whole race at
/// the end. The entry order is shuffled once and reused for every lap; lap `L`
/// opens its wave a full interval after the gun for `L + 1`, so no derived lap
/// time is zero. Each offset stays inside the interval's first half. A refusal
/// to start, or a refused stop at the end, comes back as `blocked`, never as a
/// panic.
#[expect(
    clippy::too_many_arguments,
    reason = "the run takes the engine, the field, the settings and the view's two hooks"
)]
pub(crate) fn run_simulation(
    engine: &mut RideEngine<Roster, SystemClock>,
    roster: &Roster,
    settings: &SimulationSettings,
    on_progress: &mut dyn FnMut(f32),
    is_cancelled: &dyn Fn() -> bool,
) -> SimOutcome {
    let now = SystemClock.now();
    let gun = back_dated_gun(now, settings.laps, settings.interval_minutes);
    if let Err(error) = engine.start(Some(gun)) {
        return blocked_outcome(&error);
    }
    let start = engine.actual_start().unwrap_or(gun);
    let plan = RunPlan::new(roster, engine.config().min_lap_s, settings);
    let mut progress = RunProgress::new(start, plan.total, on_progress, is_cancelled);
    for lap in 0..plan.laps {
        if plan.run_lap(engine, roster, lap, &mut progress) {
            break;
        }
    }
    if let Err(error) = engine.stop() {
        return progress.outcome(Some(vec![error.to_string()]));
    }
    progress.outcome(None)
}

/// One run's fixed script: the shuffled order, the stopped set and the gap.
struct RunPlan {
    /// One fixed place per entry, in the run's shuffled order.
    slots: Vec<Slot>,
    /// The plates of the team riders who stop after the fourth lap.
    stopped: HashSet<String>,
    /// The empty stopped set, for the laps before the stop takes effect.
    no_stopped: HashSet<String>,
    /// The short-lap gap, in seconds.
    gap: i64,
    /// How many laps to run.
    laps: usize,
    /// The minutes between one lap and the next.
    interval_minutes: i64,
    /// The attempted positions across the whole run.
    total: usize,
}

impl RunPlan {
    /// Build the plan for *roster* under *settings*.
    fn new(roster: &Roster, min_lap_s: u32, settings: &SimulationSettings) -> Self {
        let mut rng = Seeded::new(settings.seed);
        let entries = roster.entries();
        let mut order: Vec<usize> = (0..entries.len()).collect();
        rng.shuffle(&mut order);
        let offsets = lap_offsets(order.len(), settings.interval_minutes);
        let short_laps = usize::try_from(settings.short_laps).unwrap_or(0);
        let lapped = usize::try_from(settings.lapped).unwrap_or(0);
        let slots: Vec<Slot> = order
            .iter()
            .enumerate()
            .filter_map(|(index, position)| {
                entries.get(*position)?;
                Some(Slot {
                    position: *position,
                    offset_minutes: offsets.get(index).copied().unwrap_or(0),
                    short: index < short_laps,
                    lapped: index < lapped,
                })
            })
            .collect();
        let laps = usize::try_from(settings.laps).unwrap_or(0);
        let total = laps.saturating_mul(slots.len());
        Self {
            slots,
            stopped: stopped_plates(roster, settings.team_stop),
            no_stopped: HashSet::new(),
            gap: short_lap_gap(min_lap_s),
            laps,
            interval_minutes: settings.interval_minutes,
            total,
        }
    }

    /// Replay one lap, and return whether the caller cancelled the run.
    #[expect(
        clippy::too_many_arguments,
        reason = "one lap takes the engine, the field, the lap number and the run's progress"
    )]
    fn run_lap(
        &self,
        engine: &mut RideEngine<Roster, SystemClock>,
        roster: &Roster,
        lap: usize,
        progress: &mut RunProgress<'_>,
    ) -> bool {
        let wave = wave_open(progress.start, lap, self.interval_minutes);
        let stopped_now = if lap >= TEAM_STOP_LAPS {
            &self.stopped
        } else {
            &self.no_stopped
        };
        let plates = plates_to_record(roster, lap, stopped_now);
        for (index, slot) in self.slots.iter().enumerate() {
            let plate = if lap == 0 && slot.lapped {
                None
            } else {
                plates.get(slot.position).and_then(|plate| plate.as_deref())
            };
            let instant = if slot.short {
                progress.base(index) + Duration::seconds(self.gap)
            } else {
                wave + Duration::seconds(slot.offset_minutes.saturating_mul(SECONDS_PER_MINUTE))
            };
            if let Some(plate) = plate
                && engine.record_crossing(plate, Some(instant)).accepted
            {
                progress.record(index, instant);
            }
            progress.tick();
            if progress.cancelled {
                return true;
            }
        }
        false
    }
}

/// One run's progress: the counts, the last instant per position and the hooks.
struct RunProgress<'a> {
    /// The gun.
    start: Timestamp,
    /// The positions attempted so far.
    completed: usize,
    /// The positions the whole run attempts.
    total: usize,
    /// The crossings the engine accepted.
    recorded: usize,
    /// Whether the caller's cancel check fired.
    cancelled: bool,
    /// Each position's last recordings, keyed by the position's slot index.
    last: HashMap<usize, Timestamp>,
    /// The view's progress hook.
    on_progress: &'a mut dyn FnMut(f32),
    /// The caller's cancel check.
    is_cancelled: &'a dyn Fn() -> bool,
}

impl<'a> RunProgress<'a> {
    /// Start watching a run of *total* positions from *start*.
    #[expect(
        clippy::too_many_arguments,
        reason = "the progress takes the gun, the total and the view's two hooks"
    )]
    fn new(
        start: Timestamp,
        total: usize,
        on_progress: &'a mut dyn FnMut(f32),
        is_cancelled: &'a dyn Fn() -> bool,
    ) -> Self {
        Self {
            start,
            completed: 0,
            total,
            recorded: 0,
            cancelled: false,
            last: HashMap::new(),
            on_progress,
            is_cancelled,
        }
    }

    /// Return position *index*'s base instant: its last recording, else the gun.
    fn base(&self, index: usize) -> Timestamp {
        self.last.get(&index).copied().unwrap_or(self.start)
    }

    /// Record *instant* as position *index*'s newest accepted crossing.
    fn record(&mut self, index: usize, instant: Timestamp) {
        self.recorded = self.recorded.saturating_add(1);
        self.last.insert(index, instant);
    }

    /// Report one attempted position and read the cancel check.
    fn tick(&mut self) {
        self.completed = self.completed.saturating_add(1);
        (self.on_progress)(fraction(self.completed, self.total));
        self.cancelled = (self.is_cancelled)();
    }

    /// Return the run's outcome with *blocked* as its refusal, when any.
    fn outcome(&self, blocked: Option<Vec<String>>) -> SimOutcome {
        SimOutcome {
            cancelled: self.cancelled,
            recorded: self.recorded,
            blocked,
        }
    }
}

/// One entry's fixed place in the run's shuffled order.
struct Slot {
    /// The entry's index in the roster's entry order.
    position: usize,
    /// The whole-minute offset inside the wave.
    offset_minutes: i64,
    /// Whether the entry crosses under the ride's minimum lap.
    short: bool,
    /// Whether the entry misses the first wave.
    lapped: bool,
}

/// Return the gun: the clock less the whole race and a half interval.
fn back_dated_gun(now: Timestamp, laps: i64, interval_minutes: i64) -> Timestamp {
    let race = laps
        .saturating_mul(interval_minutes)
        .saturating_mul(SECONDS_PER_MINUTE);
    let half = interval_minutes.saturating_mul(SECONDS_PER_MINUTE) / 2;
    let back = race.saturating_add(half);
    Timestamp::from_unix_timestamp(now.unix_timestamp().saturating_sub(back)).unwrap_or(now)
}

/// Return the instant that lap `lap`'s wave opens.
fn wave_open(start: Timestamp, lap: usize, interval_minutes: i64) -> Timestamp {
    let waves = i64::try_from(lap.saturating_add(1))
        .unwrap_or(i64::MAX)
        .saturating_mul(interval_minutes);
    start.saturating_add(Duration::seconds(waves.saturating_mul(SECONDS_PER_MINUTE)))
}

/// Return the progress fraction for *completed* of *total* positions.
#[expect(
    clippy::cast_precision_loss,
    reason = "the fraction is a display value, and a run's position count is far below f32's exact range"
)]
fn fraction(completed: usize, total: usize) -> f32 {
    completed as f32 / total.max(1) as f32
}

/// Return the outcome of a refused start.
fn blocked_outcome(error: &RideError) -> SimOutcome {
    let reasons = match error {
        RideError::StartBlocked { reasons, .. } => reasons.clone(),
        other => vec![other.to_string()],
    };
    SimOutcome {
        cancelled: false,
        recorded: 0,
        blocked: Some(reasons),
    }
}

/// Return the plates one simulated lap records, one per entry, in roster order.
///
/// A relay ride crosses under the entry's own plate, so a stopped rider changes
/// nothing. A pooled ride crosses under one rider's own plate: a solo's, or,
/// for a team, the plate of the rider on course that lap. `None` marks a
/// position with no one left to cross.
fn plates_to_record(
    roster: &Roster,
    lap: usize,
    stopped_plates: &HashSet<String>,
) -> Vec<Option<String>> {
    if roster.plate_model() == PlateModel::TeamRelay {
        return roster
            .entries()
            .iter()
            .map(|entry| Some(entry.plate.clone()))
            .collect();
    }
    roster
        .entries()
        .iter()
        .map(|entry| pooled_plate(entry, lap, stopped_plates))
        .collect()
}

/// Return one entry's crossing plate on *lap* under the pooled model.
///
/// A solo entry crosses under its own plate; a team sends one rider per lap,
/// rotating round-robin through its members. A rotation slot landing on a
/// stopped rider passes that lap to the team's next active rider; a team whose
/// every rider has stopped sends nobody. A riderless team falls back to its own
/// provisional plate claim.
fn pooled_plate(entry: &Entry, lap: usize, stopped_plates: &HashSet<String>) -> Option<String> {
    let riders = &entry.riders;
    if entry.kind == EntryType::Solo || riders.is_empty() {
        return Some(entry.plate.clone());
    }
    let start = lap % riders.len();
    riders
        .iter()
        .cycle()
        .skip(start)
        .take(riders.len())
        .filter_map(|rider| rider.plate.as_deref())
        .find(|plate| !stopped_plates.contains(*plate))
        .map(str::to_owned)
}

/// Return the plates of the first `count` team riders.
///
/// The pick is deterministic: the team entries' riders in roster order, so one
/// seed reproduces a run. A count below one stops nobody, and a plateless rider
/// has no plate to stop.
fn stopped_plates(roster: &Roster, count: i64) -> HashSet<String> {
    if count < 1 {
        return HashSet::new();
    }
    let take = usize::try_from(count).unwrap_or(0);
    team_entries(roster)
        .into_iter()
        .flat_map(|entry| entry.riders.iter())
        .take(take)
        .filter_map(|rider| rider.plate.clone())
        .collect()
}

/// Return each entry position's whole-minute offset inside one wave.
///
/// The offsets spread evenly across `0 .. interval // 2`, so the wave's last
/// crossing sits on or before the half-interval mark. A one-entry wave sits at
/// its own start instead.
fn lap_offsets(entry_count: usize, interval_minutes: i64) -> Vec<i64> {
    if entry_count <= 1 {
        return vec![0; entry_count];
    }
    let span = interval_minutes / 2;
    let last = i64::try_from(entry_count - 1).unwrap_or(1);
    (0..entry_count)
        .map(|index| i64::try_from(index).unwrap_or(0) * span / last)
        .collect()
}

/// Return the roster's live team entries, in creation order.
fn team_entries(roster: &Roster) -> Vec<&Entry> {
    roster
        .entries()
        .iter()
        .filter(|entry| entry.kind == EntryType::Team)
        .collect()
}

/// Create the `counter`-th generated solo entry.
fn create_solo(roster: &mut Roster, counter: usize, rng: &mut Seeded) -> Result<(), String> {
    let plate = roster.next_free_plate();
    let rider = generated_rider(counter, Some(plate.clone()), rng);
    roster
        .create_solo_entry(rider, &plate)
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Attach the `counter`-th generated rider to `entry`.
///
/// A pooled ride gives every rider their own plate; a relay ride leaves them
/// plateless, because the entry owns the plate.
#[expect(
    clippy::too_many_arguments,
    reason = "the generator passes the roster, the team, the counter, the plate policy and its generator"
)]
fn add_team_rider(
    roster: &mut Roster,
    entry: &EntryId,
    counter: usize,
    pooled: bool,
    rng: &mut Seeded,
) -> Result<(), String> {
    let plate = if pooled {
        Some(roster.next_free_plate())
    } else {
        None
    };
    let rider = generated_rider(counter, plate, rng);
    roster
        .add_rider_to_team(rider, entry)
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Return the `counter`-th generated placeholder rider.
fn generated_rider(counter: usize, plate: Option<String>, rng: &mut Seeded) -> Rider {
    Rider {
        first_name: format!("FIRSTNAME-{counter:04}"),
        last_name: format!("LASTNAME-{counter:04}"),
        plate,
        sex: Some(rng.sex().to_owned()),
        sort_order: 0,
    }
}

/// Refuse generator inputs outside their own bounds.
fn require_generator_inputs(input: &GeneratorInput, max_team_size: i64) -> Result<(), String> {
    if input.total < 1 {
        return Err("total must be at least 1".to_owned());
    }
    if input.teams < 1 {
        return Err("teams must be at least 1".to_owned());
    }
    if input.solo < 0 || input.solo > input.total {
        return Err(format!("solo must be between 0 and {}", input.total));
    }
    let team_riders = input.total.saturating_sub(input.solo);
    let lowest = i64::from(SMALLEST_TEAM).saturating_mul(input.teams);
    let highest = max_team_size.saturating_mul(input.teams);
    if !(lowest..=highest).contains(&team_riders) {
        return Err(format!(
            "team riders must be between {lowest} and {highest}, got {team_riders}"
        ));
    }
    Ok(())
}

/// A 64-bit linear congruential generator, so one seed reproduces a run.
struct Seeded {
    /// The generator's current state.
    state: u64,
}

impl Seeded {
    /// Return a generator seeded with `seed`.
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Return the next value.
    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(LCG_MULTIPLIER)
            .wrapping_add(LCG_INCREMENT);
        self.state
    }

    /// Return a value below `bound`.
    fn below(&mut self, bound: usize) -> usize {
        let bound = bound.max(1);
        usize::try_from(self.next_u64() >> 33).unwrap_or(0) % bound
    }

    /// Return one of the two sexes a generated rider may carry.
    fn sex(&mut self) -> &'static str {
        SEXES.get(self.below(SEXES.len())).copied().unwrap_or("M")
    }

    /// Shuffle `order` in place from the back, Fisher-Yates style.
    fn shuffle(&mut self, order: &mut [usize]) {
        for index in (1..order.len()).rev() {
            let swap = self.below(index + 1);
            order.swap(index, swap);
        }
    }
}
