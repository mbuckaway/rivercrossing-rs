// SPDX-License-Identifier: GPL-3.0-only

//! The pure import preview: read the roster CSV, resolve its header, parse the
//! rows and assemble the entries, conflicts and warnings (R-21, R-24, R-78).
//!
//! The preview takes the roster by shared reference and writes nothing, so the
//! preview-then-commit pair never leaves a partial import behind. The commit
//! applies the parsed entries through the roster's audited mutators.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use rivercrossing_roster::enums::{EntryMode, EntryType, PlateModel, RideStatus};
use rivercrossing_roster::lock::can_edit_structure;
use rivercrossing_roster::names::{rider_name_key, team_name_key};
use rivercrossing_roster::roster::{MIN_TEAM_SIZE, Roster};
use rivercrossing_roster::types::{Entry, Rider};

use crate::error::CsvIoError;
use crate::header::{self, HeaderMap};
use crate::model::{
    ImportOptions, ImportPreview, ImportProblem, ParsedEntry, ParsedRider, PreviewFingerprint,
    ProblemCode,
};
use crate::normalize::{self, SexCell};
use crate::wire;

/// The one-based row number of the header.
const HEADER_ROW: usize = 1;

/// The one-based row number of the first data row.
const FIRST_DATA_ROW: usize = 2;

/// The conflict text for a file whose header names no rider-name column.
const HEADER_PROBLEM: &str = "missing or malformed header: no first or last name column";

/// The conflict text for a row that names no rider.
const MISSING_NAME_PROBLEM: &str = "missing name";

/// The conflict text for a team row on a solo-only ride (R-11).
const SOLO_ONLY_TEAM_PROBLEM: &str = "team entries are not allowed on a solo-only ride";

// ----------------------------------------------------------- public seams

/// Preview a roster CSV file against *roster* without writing anything.
///
/// The file's own header decides where each canonical field lives; a header
/// that names no first or last name column reports one row-1 problem and
/// parses no rows (spec §7).
///
/// # Errors
///
/// Return [`CsvIoError::Io`] when *path* cannot be read, [`CsvIoError::NotUtf8`]
/// when the payload is not valid `UTF-8`, and [`CsvIoError::Malformed`] when the
/// CSV reader refuses the payload.
pub fn preview(
    path: &Path,
    roster: &Roster,
    options: ImportOptions,
) -> Result<ImportPreview, CsvIoError> {
    let bytes = std::fs::read(path)?;
    preview_bytes(path, &bytes, roster, options)
}

/// Preview an in-memory payload under the name *source*.
#[expect(
    clippy::too_many_arguments,
    reason = "the seam fixes the source name, the payload, the roster and the two options; a parameter struct would add a type with one caller"
)]
pub(crate) fn preview_bytes(
    source: &Path,
    bytes: &[u8],
    roster: &Roster,
    options: ImportOptions,
) -> Result<ImportPreview, CsvIoError> {
    let records = wire::read_records(bytes)?;
    let Some((header_row, data_rows)) = records.split_first() else {
        return Ok(whole_file_conflict(
            source,
            roster,
            problem(HEADER_ROW, ProblemCode::Header, HEADER_PROBLEM),
        ));
    };
    let mapping = match header::resolve(header_row) {
        Ok(mapping) => mapping,
        Err(message) => {
            return Ok(whole_file_conflict(
                source,
                roster,
                problem(HEADER_ROW, ProblemCode::Header, &message),
            ));
        }
    };
    let reader = RowReader {
        mapping: &mapping,
        options,
    };
    let parts = assemble_all(&reader, data_rows, roster);
    Ok(ImportPreview {
        source_path: source.to_path_buf(),
        riders: parts.entries.iter().map(|entry| entry.riders.len()).sum(),
        teams: parts
            .entries
            .iter()
            .filter(|entry| entry.kind == EntryType::Team)
            .count(),
        entries: parts.entries,
        conflicts: parts.conflicts,
        warnings: parts.warnings,
        fingerprint: fingerprint(roster),
    })
}

/// The entries and problems of one whole preview pass.
struct PreviewParts {
    /// The assembled entries.
    entries: Vec<ParsedEntry>,
    /// The blocking conflicts, in row order.
    conflicts: Vec<ImportProblem>,
    /// The non-blocking warnings, in row order.
    warnings: Vec<ImportProblem>,
}

/// Parse, reject, assemble and merge the findings of one pass.
fn assemble_all(reader: &RowReader<'_>, records: &[Vec<String>], roster: &Roster) -> PreviewParts {
    let (rows, row_conflicts) = read_data_rows(records, reader);
    let (accepted, solo_conflicts) = reject_team_rows(rows, roster);
    let converted = converted_team_names(&accepted, roster, reader.options);
    let assembly = assemble(&accepted, roster, reader.options);
    let mut conflicts = row_conflicts;
    conflicts.extend(solo_conflicts);
    conflicts.extend(assembly.conflicts);
    conflicts.sort_by_key(|problem| problem.row);
    let mut warnings = assembly.warnings;
    warnings.extend(duplicate_rider_warnings(&accepted));
    warnings.extend(near_duplicate_team_warnings(&accepted, &converted));
    warnings.sort_by_key(|problem| problem.row);
    PreviewParts {
        entries: assembly.entries,
        conflicts,
        warnings,
    }
}

// -------------------------------------------------------------- preview glue

/// Return the empty preview for a whole-file problem at row 1.
fn whole_file_conflict(source: &Path, roster: &Roster, problem: ImportProblem) -> ImportPreview {
    ImportPreview {
        source_path: source.to_path_buf(),
        riders: 0,
        teams: 0,
        entries: Vec::new(),
        conflicts: vec![problem],
        warnings: Vec::new(),
        fingerprint: fingerprint(roster),
    }
}

/// Return the roster state that a commit must still match.
fn fingerprint(roster: &Roster) -> PreviewFingerprint {
    PreviewFingerprint {
        audit_len: roster.audit_log().len(),
        entries: roster.entries().len(),
        riders: roster.entries().iter().map(Entry::team_size).sum(),
    }
}

/// Build one preview problem.
fn problem(row: usize, code: ProblemCode, message: &str) -> ImportProblem {
    ImportProblem {
        row,
        code,
        message: message.to_owned(),
    }
}

// ----------------------------------------------------------- row parsing

/// One parsed data row, before assembly into an entry.
struct DataRow {
    /// The one-based file row.
    row: usize,
    /// The canonical first name.
    first_name: String,
    /// The canonical last name.
    last_name: String,
    /// Whether the row names a team.
    is_team: bool,
    /// The team name as the file spells it, whitespace-collapsed.
    team_name: String,
    /// The team merge key.
    team_key: String,
    /// The explicit plate cell, or an empty string.
    plate: String,
    /// The row's notes.
    notes: String,
    /// The folded sex, when the row names a known one.
    sex: Option<String>,
}

impl DataRow {
    /// Return the rider's full display name.
    fn full_name(&self) -> String {
        full_name(&self.first_name, &self.last_name)
    }
}

/// The header map and options that every row parse consults.
struct RowReader<'a> {
    /// The resolved header columns.
    mapping: &'a HeaderMap,
    /// The operator's opt-ins.
    options: ImportOptions,
}

impl RowReader<'_> {
    /// Parse one data row, or report why it is excluded.
    fn parse(&self, row: usize, record: &[String]) -> Result<Option<DataRow>, ImportProblem> {
        let first = cell(record, self.mapping.first_name);
        let last = cell(record, self.mapping.last_name);
        let type_cell = cell(record, self.mapping.kind);
        let team_raw = cell(record, self.mapping.team_name);
        let plate = cell(record, self.mapping.plate);
        let notes = cell(record, self.mapping.notes);
        let sex_raw = cell(record, self.mapping.sex);
        if first.is_empty() && last.is_empty() {
            if plate.is_empty() && team_raw.is_empty() && type_cell.is_empty() {
                return Ok(None);
            }
            return Err(problem(row, ProblemCode::MissingName, MISSING_NAME_PROBLEM));
        }
        let sex = parse_sex(row, &sex_raw, self.options)?;
        let team_name = normalize::display_team_name(&team_raw);
        let kind = parse_kind(row, &type_cell, &team_name)?;
        let is_team = kind == EntryType::Team;
        Ok(Some(DataRow {
            row,
            first_name: normalize::canonical_person_name(&first),
            last_name: normalize::canonical_person_name(&last),
            is_team,
            team_name: if is_team { team_name } else { String::new() },
            team_key: if is_team {
                normalize::normalize_team_name(&team_raw)
            } else {
                String::new()
            },
            plate,
            notes,
            sex,
        }))
    }
}

/// Return *field*'s trimmed cell, or an empty string when it is unmapped.
fn cell(record: &[String], index: Option<usize>) -> String {
    index
        .and_then(|index| record.get(index))
        .map_or_else(String::new, |value| value.trim().to_owned())
}

/// Fold the sex cell, or report an unrecognised value.
fn parse_sex(
    row: usize,
    raw: &str,
    options: ImportOptions,
) -> Result<Option<String>, ImportProblem> {
    match normalize::classify_sex(raw, options.map_unknown_sex_to_male) {
        SexCell::Known(sex) => Ok(sex),
        SexCell::Invalid(value) => Err(problem(
            row,
            ProblemCode::InvalidSex,
            &format!("invalid sex {value:?}"),
        )),
    }
}

/// Derive the entry kind, or report a missing name or an unknown type.
fn parse_kind(row: usize, type_cell: &str, team_name: &str) -> Result<EntryType, ImportProblem> {
    match normalize::derive_kind(type_cell, team_name) {
        Ok(kind) => Ok(kind),
        Err(message) if message == MISSING_NAME_PROBLEM => {
            Err(problem(row, ProblemCode::MissingName, MISSING_NAME_PROBLEM))
        }
        Err(message) => Err(problem(row, ProblemCode::UnknownType, &message)),
    }
}

/// Parse every data row of *records*, numbering from row 2.
fn read_data_rows(
    records: &[Vec<String>],
    reader: &RowReader<'_>,
) -> (Vec<DataRow>, Vec<ImportProblem>) {
    let mut rows = Vec::new();
    let mut conflicts = Vec::new();
    for (offset, record) in records.iter().enumerate() {
        let row = FIRST_DATA_ROW + offset;
        match reader.parse(row, record) {
            Ok(Some(parsed)) => rows.push(parsed),
            Ok(None) => {}
            Err(problem) => conflicts.push(problem),
        }
    }
    (rows, conflicts)
}

/// Drop each team row of a solo-only ride, with one conflict each (R-11).
fn reject_team_rows(rows: Vec<DataRow>, roster: &Roster) -> (Vec<DataRow>, Vec<ImportProblem>) {
    if roster.entry_mode() != EntryMode::Solo {
        return (rows, Vec::new());
    }
    let mut accepted = Vec::new();
    let mut conflicts = Vec::new();
    for row in rows {
        if row.is_team {
            conflicts.push(problem(
                row.row,
                ProblemCode::SoloOnlyTeam,
                SOLO_ONLY_TEAM_PROBLEM,
            ));
        } else {
            accepted.push(row);
        }
    }
    (accepted, conflicts)
}

/// Return the team keys that the convert opt-in turns into solo entries.
fn converted_team_names(
    rows: &[DataRow],
    roster: &Roster,
    options: ImportOptions,
) -> BTreeSet<String> {
    if !options.convert_teams_of_one_to_solo || roster.status() != RideStatus::Draft {
        return BTreeSet::new();
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for row in rows.iter().filter(|row| row.is_team) {
        *counts.entry(row.team_key.as_str()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .filter(|(_, count)| *count == 1)
        .map(|(key, _)| key.to_owned())
        .collect()
}

// ------------------------------------------------------------- allocation

/// The sequential plate allocator for blank PLATE cells (R-20, spec §7).
///
/// The roster's own next-free probe already skips every plate on the roster and
/// every plate the file names, and each allocation advances by one, so no
/// allocation can collide.
struct PlateAllocator {
    /// The next plate to hand out.
    next: String,
}

impl PlateAllocator {
    /// Build the allocator from the roster's plates and every explicit cell.
    fn new(roster: &Roster, rows: &[DataRow]) -> Self {
        let reserved = reserved_plates(rows);
        Self {
            next: roster.next_free_plate_skipping(&reserved),
        }
    }

    /// Return the next free sequential plate and advance the allocator.
    fn allocate(&mut self) -> String {
        let plate = self.next.clone();
        self.next = next_plate(&plate);
        plate
    }
}

/// Return every non-blank plate cell that the file names.
fn reserved_plates(rows: &[DataRow]) -> BTreeSet<String> {
    rows.iter()
        .filter(|row| !row.plate.is_empty())
        .map(|row| row.plate.clone())
        .collect()
}

/// Return the canonical decimal successor of a whole-number plate.
fn next_plate(digits: &str) -> String {
    let trimmed = digits.trim_start_matches('0');
    let mut incremented: Vec<u8> = trimmed
        .bytes()
        .map(|byte| byte.saturating_sub(b'0'))
        .collect();
    let mut carry = true;
    for value in incremented.iter_mut().rev() {
        if !carry {
            break;
        }
        if *value < 9 {
            *value += 1;
            carry = false;
        } else {
            *value = 0;
        }
    }
    if carry {
        incremented.insert(0, 1);
    }
    incremented
        .into_iter()
        .map(|value| char::from(b'0' + value))
        .collect()
}

/// Compare two whole-number plates by value, then lexicographically.
fn compare_plates(left: &str, right: &str) -> Ordering {
    let trimmed_left = left.trim_start_matches('0');
    let trimmed_right = right.trim_start_matches('0');
    (trimmed_left.len(), trimmed_left)
        .cmp(&(trimmed_right.len(), trimmed_right))
        .then_with(|| left.cmp(right))
}

/// Return the numerically lowest of a team's member plates.
fn lowest_plate(rows: &[PooledRow]) -> String {
    rows.iter()
        .filter_map(|row| row.rider.plate.as_deref())
        .min_by(|left, right| compare_plates(left, right))
        .unwrap_or_default()
        .to_owned()
}

/// Return the plate for *row*, allocating one when the cell is blank.
fn plate_for(allocator: &mut PlateAllocator, row: &DataRow) -> String {
    if row.plate.is_empty() {
        return allocator.allocate();
    }
    row.plate.clone()
}

/// Return whether *plate* is a non-blank, non-whole-number cell.
fn explicit_non_digit_plate(plate: &str) -> bool {
    !plate.is_empty() && !plate.bytes().all(|byte| byte.is_ascii_digit())
}

/// Return the conflict text for a non-whole-number plate cell.
fn non_digit_message(plate: &str) -> String {
    format!("plate {plate:?} must be a whole number")
}

/// Return the conflict text for a plate that repeats in the file.
fn duplicate_plate_message(plate: &str) -> String {
    format!("duplicate plate {plate}")
}

// --------------------------------------------------------------- assembly

/// The parsed entries and the non-blocking and blocking findings of one pass.
#[derive(Default)]
struct Assembly {
    /// The assembled entries.
    entries: Vec<ParsedEntry>,
    /// The blocking conflicts.
    conflicts: Vec<ImportProblem>,
    /// The non-blocking warnings.
    warnings: Vec<ImportProblem>,
}

/// One assembled item: the entry it produced and its findings.
struct Outcome {
    /// The one-based anchor row.
    anchor: usize,
    /// The entry, or `None` when the item contributes no entry.
    entry: Option<ParsedEntry>,
    /// The blocking conflicts.
    conflicts: Vec<ImportProblem>,
    /// The non-blocking warnings.
    warnings: Vec<ImportProblem>,
}

impl Outcome {
    /// Build the outcome of an item that contributes no entry.
    fn refused(anchor: usize, code: ProblemCode, message: &str) -> Self {
        Self {
            anchor,
            entry: None,
            conflicts: vec![problem(anchor, code, message)],
            warnings: Vec::new(),
        }
    }
}

/// Return the team-size finding for one team.
struct SizeOutcome {
    /// The blocking conflicts.
    conflicts: Vec<ImportProblem>,
    /// The non-blocking warnings.
    warnings: Vec<ImportProblem>,
}

/// Return the size problem of a team, as a warning only while DRAFT (R-78).
fn team_size_outcome(entry: &ParsedEntry, roster: &Roster, anchor: usize) -> SizeOutcome {
    let size = u32::try_from(entry.riders.len()).unwrap_or(u32::MAX);
    if let Some(message) = team_under_min_message(size) {
        let finding = problem(anchor, ProblemCode::TeamUnderMin, &message);
        return if roster.status() == RideStatus::Draft {
            SizeOutcome {
                conflicts: Vec::new(),
                warnings: vec![finding],
            }
        } else {
            SizeOutcome {
                conflicts: vec![finding],
                warnings: Vec::new(),
            }
        };
    }
    SizeOutcome {
        conflicts: team_over_max_message(size, roster.max_team_size())
            .map(|message| problem(anchor, ProblemCode::TeamOverMax, &message))
            .into_iter()
            .collect(),
        warnings: Vec::new(),
    }
}

/// Return the under-min team message, or `None` at the floor.
fn team_under_min_message(size: u32) -> Option<String> {
    if size >= MIN_TEAM_SIZE {
        return None;
    }
    Some(format!(
        "team of {size} rider is below the minimum of {MIN_TEAM_SIZE} (team-under-min)"
    ))
}

/// Return the over-max team message, or `None` within the ceiling.
fn team_over_max_message(size: u32, max_team_size: u32) -> Option<String> {
    if size <= max_team_size {
        return None;
    }
    Some(format!(
        "team of {size} riders exceeds the maximum of {max_team_size} (team-over-max)"
    ))
}

/// Assemble *rows* under the ride's plate model.
fn assemble(rows: &[DataRow], roster: &Roster, options: ImportOptions) -> Assembly {
    match roster.plate_model() {
        PlateModel::TeamRelay => RelayAssembler::new(roster, rows, options).run(rows),
        PlateModel::RiderPooled => PooledAssembler::new(roster, rows, options).run(rows),
    }
}

// ----------------------------------------------------------- relay model

/// One planned relay item: a solo row or a team group anchored at its first row.
enum PlanItem<'a> {
    /// One solo row.
    Solo(&'a DataRow),
    /// A team group with its first row.
    Group(&'a DataRow, Vec<&'a DataRow>),
}

/// Group the team rows by their merge key.
fn team_groups(rows: &[DataRow]) -> BTreeMap<&str, Vec<&DataRow>> {
    let mut groups: BTreeMap<&str, Vec<&DataRow>> = BTreeMap::new();
    for row in rows.iter().filter(|row| row.is_team) {
        groups.entry(row.team_key.as_str()).or_default().push(row);
    }
    groups
}

/// Return the relay entries in file order, each team anchored at its first row.
fn relay_plan(rows: &[DataRow]) -> Vec<PlanItem<'_>> {
    let mut remaining = team_groups(rows);
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut plan = Vec::new();
    for row in rows {
        if !row.is_team {
            plan.push(PlanItem::Solo(row));
            continue;
        }
        if !seen.insert(row.team_key.as_str()) {
            continue;
        }
        let group = remaining.remove(row.team_key.as_str()).unwrap_or_default();
        plan.push(PlanItem::Group(row, group));
    }
    plan
}

/// The mutable state of one `team_relay` assembly pass.
struct RelayAssembler<'a> {
    /// The roster the pass previews against.
    roster: &'a Roster,
    /// The operator's opt-ins.
    options: ImportOptions,
    /// The blank-plate allocator.
    allocator: PlateAllocator,
    /// The entry plates the file has produced so far.
    seen_plates: BTreeSet<String>,
    /// The assembled entries and findings.
    assembly: Assembly,
}

impl<'a> RelayAssembler<'a> {
    /// Build the pass for *rows*.
    fn new(roster: &'a Roster, rows: &[DataRow], options: ImportOptions) -> Self {
        Self {
            roster,
            options,
            allocator: PlateAllocator::new(roster, rows),
            seen_plates: BTreeSet::new(),
            assembly: Assembly::default(),
        }
    }

    /// Assemble every planned item.
    fn run(mut self, rows: &[DataRow]) -> Assembly {
        for item in relay_plan(rows) {
            let outcome = match item {
                PlanItem::Solo(row) => self.solo(row),
                PlanItem::Group(row, group) => self.team(row, &group),
            };
            self.apply(outcome);
        }
        self.assembly
    }

    /// Build one solo entry.
    fn solo(&mut self, row: &DataRow) -> Outcome {
        let anchor = row.row;
        if explicit_non_digit_plate(&row.plate) {
            return Outcome::refused(
                anchor,
                ProblemCode::NonDigitPlate,
                &non_digit_message(&row.plate),
            );
        }
        let plate = plate_for(&mut self.allocator, row);
        let entry = solo_entry(row, &plate, &row.notes);
        let conflicts = relay_structural_problem(self.roster, &plate, &entry)
            .map(|message| problem(anchor, ProblemCode::BlockedReshape, &message))
            .into_iter()
            .collect();
        Outcome {
            anchor,
            entry: Some(entry),
            conflicts,
            warnings: Vec::new(),
        }
    }

    /// Build one team entry, or report why the group contributes none.
    fn team(&mut self, first: &DataRow, group: &[&DataRow]) -> Outcome {
        let anchor = first.row;
        let plate = match self.team_plate(anchor, group) {
            Ok(plate) => plate,
            Err(problem) => {
                return Outcome {
                    anchor,
                    entry: None,
                    conflicts: vec![problem],
                    warnings: Vec::new(),
                };
            }
        };
        let riders: Vec<ParsedRider> = group.iter().map(|row| parsed_rider(row, None)).collect();
        if let (true, [rider]) = (self.options.convert_teams_of_one_to_solo, riders.as_slice()) {
            let entry = solo_entry_from_rider(rider, &plate, &first.notes);
            if let Some(outcome) = self.converted(anchor, &entry) {
                return outcome;
            }
        }
        let entry = ParsedEntry {
            plate,
            display_name: first.team_name.clone(),
            kind: EntryType::Team,
            riders,
            notes: grouped_notes(group),
        };
        let size = team_size_outcome(&entry, self.roster, anchor);
        let mut conflicts = size.conflicts;
        if conflicts.is_empty()
            && let Some(message) = relay_structural_problem(self.roster, &entry.plate, &entry)
        {
            conflicts.push(problem(anchor, ProblemCode::BlockedReshape, &message));
        }
        Outcome {
            anchor,
            entry: Some(entry),
            conflicts,
            warnings: size.warnings,
        }
    }

    /// Return the group's one shared plate, or the reason it has none.
    fn team_plate(&mut self, anchor: usize, group: &[&DataRow]) -> Result<String, ImportProblem> {
        let group_plates = group_plates(group);
        if group_plates.len() > 1 {
            return Err(problem(
                anchor,
                ProblemCode::TeamRowsDifferentPlates,
                &plate_disagreement_message(&group_plates),
            ));
        }
        let explicit = group_plates.first().copied().unwrap_or_default();
        if explicit_non_digit_plate(explicit) {
            return Err(problem(
                anchor,
                ProblemCode::NonDigitPlate,
                &non_digit_message(explicit),
            ));
        }
        if explicit.is_empty() {
            return Ok(self.allocator.allocate());
        }
        Ok(explicit.to_owned())
    }

    /// Return the solo entry the convert opt-in asks for, or `None` to fall through.
    fn converted(&self, anchor: usize, entry: &ParsedEntry) -> Option<Outcome> {
        let refused = relay_structural_problem(self.roster, &entry.plate, entry);
        if self.roster.status() == RideStatus::Draft && refused.is_none() {
            return Some(Outcome {
                anchor,
                entry: Some(entry.clone()),
                conflicts: Vec::new(),
                warnings: Vec::new(),
            });
        }
        refused.map(|message| Outcome::refused(anchor, ProblemCode::BlockedReshape, &message))
    }

    /// Keep the outcome's entry, flagging a plate that repeats in the file.
    fn apply(&mut self, outcome: Outcome) {
        let Some(entry) = outcome.entry else {
            self.assembly.conflicts.extend(outcome.conflicts);
            return;
        };
        let plate = entry.plate.clone();
        if !self.seen_plates.insert(plate.clone()) {
            self.assembly.conflicts.push(problem(
                outcome.anchor,
                ProblemCode::DuplicatePlate,
                &duplicate_plate_message(&plate),
            ));
        }
        self.assembly.entries.push(entry);
        self.assembly.conflicts.extend(outcome.conflicts);
        self.assembly.warnings.extend(outcome.warnings);
    }
}

/// Return the distinct non-blank plates that a team group names.
fn group_plates<'a>(group: &[&'a DataRow]) -> BTreeSet<&'a str> {
    group
        .iter()
        .map(|row| row.plate.as_str())
        .filter(|plate| !plate.is_empty())
        .collect()
}

/// Return the conflict text for team rows that name different plates.
fn plate_disagreement_message(plates: &BTreeSet<&str>) -> String {
    let ordered = plates.iter().copied().collect::<Vec<_>>().join(", ");
    format!("team rows carry different plates ({ordered})")
}

/// Return whether a matched relay entry's shape may not change now (R-17).
fn relay_structural_problem(roster: &Roster, plate: &str, parsed: &ParsedEntry) -> Option<String> {
    let existing = roster.entries().iter().find(|entry| entry.plate == plate)?;
    if !relay_composition_changed(existing, parsed) {
        return None;
    }
    if can_edit_structure(roster.status()) {
        return None;
    }
    Some(format!(
        "team composition changed but the ride is {}; only new plates or name fixes are allowed",
        roster.status().as_str()
    ))
}

/// Return whether *parsed* changes the matched entry's kind or team roster.
fn relay_composition_changed(existing: &Entry, parsed: &ParsedEntry) -> bool {
    if existing.kind != parsed.kind {
        return true;
    }
    if parsed.kind == EntryType::Solo {
        return false;
    }
    let existing_names: Vec<String> = existing.riders.iter().map(Rider::full_name).collect();
    let parsed_names: Vec<String> = parsed.riders.iter().map(rider_full_name).collect();
    existing_names != parsed_names
}

// ---------------------------------------------------------- pooled model

/// One `rider_pooled` team member row, after plate assignment.
struct PooledRow {
    /// The one-based file row.
    row: usize,
    /// The parsed rider, carrying their own plate.
    rider: ParsedRider,
    /// The row's notes.
    notes: String,
}

/// The mutable state of one `rider_pooled` assembly pass.
struct PooledAssembler<'a> {
    /// The roster the pass previews against.
    roster: &'a Roster,
    /// The operator's opt-ins.
    options: ImportOptions,
    /// Every roster rider plate and the entry that claims it.
    index: BTreeMap<&'a str, &'a Entry>,
    /// The blank-plate allocator.
    allocator: PlateAllocator,
    /// The first row of each team key, in first-seen order.
    order: Vec<&'a DataRow>,
    /// Each team key's member rows.
    groups: BTreeMap<&'a str, Vec<PooledRow>>,
    /// Every plate the file has produced so far.
    seen_plates: BTreeSet<String>,
    /// The assembled entries and findings.
    assembly: Assembly,
}

impl<'a> PooledAssembler<'a> {
    /// Build the pass for *rows*.
    fn new(roster: &'a Roster, rows: &'a [DataRow], options: ImportOptions) -> Self {
        Self {
            roster,
            options,
            index: pooled_owner_index(roster),
            allocator: PlateAllocator::new(roster, rows),
            order: Vec::new(),
            groups: BTreeMap::new(),
            seen_plates: BTreeSet::new(),
            assembly: Assembly::default(),
        }
    }

    /// Assemble the solo entries, then the team entries.
    fn run(mut self, rows: &'a [DataRow]) -> Assembly {
        self.collect_rows(rows);
        self.collect_teams();
        self.assembly
    }

    /// Parse every row: solos become entries, team rows are grouped.
    fn collect_rows(&mut self, rows: &'a [DataRow]) {
        for row in rows {
            if explicit_non_digit_plate(&row.plate) {
                self.assembly.conflicts.push(problem(
                    row.row,
                    ProblemCode::NonDigitPlate,
                    &non_digit_message(&row.plate),
                ));
                continue;
            }
            let plate = plate_for(&mut self.allocator, row);
            if !self.seen_plates.insert(plate.clone()) {
                self.assembly.conflicts.push(problem(
                    row.row,
                    ProblemCode::DuplicatePlate,
                    &duplicate_plate_message(&plate),
                ));
            }
            let rider = parsed_rider(row, Some(&plate));
            if row.is_team {
                self.group_row(row, rider);
                continue;
            }
            if let Some(message) = pooled_solo_problem(&self.index, &plate, self.roster.status()) {
                self.assembly.conflicts.push(problem(
                    row.row,
                    ProblemCode::BlockedReshape,
                    &message,
                ));
            }
            self.assembly
                .entries
                .push(solo_entry_from_rider(&rider, &plate, &row.notes));
        }
    }

    /// Group one team row under its key, recording a new key's first row.
    fn group_row(&mut self, row: &'a DataRow, rider: ParsedRider) {
        let key = row.team_key.as_str();
        if !self.groups.contains_key(key) {
            self.order.push(row);
        }
        self.groups.entry(key).or_default().push(PooledRow {
            row: row.row,
            rider,
            notes: row.notes.clone(),
        });
    }

    /// Build one team entry per team key, in first-seen order.
    fn collect_teams(&mut self) {
        let order = std::mem::take(&mut self.order);
        for first in order {
            let group = self
                .groups
                .remove(first.team_key.as_str())
                .unwrap_or_default();
            let outcome = self.team(first, &group);
            self.push(outcome);
        }
    }

    /// Build one team entry, or the solo entry the opt-in converts it to.
    fn team(&mut self, first: &'a DataRow, group: &[PooledRow]) -> Outcome {
        let anchor = first.row;
        if let (true, [only]) = (self.options.convert_teams_of_one_to_solo, group) {
            let plate = only.rider.plate.clone().unwrap_or_default();
            let entry = solo_entry_from_rider(&only.rider, &plate, &only.notes);
            let refused = pooled_solo_problem(&self.index, &plate, self.roster.status());
            if self.roster.status() == RideStatus::Draft && refused.is_none() {
                return Outcome {
                    anchor,
                    entry: Some(entry),
                    conflicts: Vec::new(),
                    warnings: Vec::new(),
                };
            }
            if let Some(message) = refused {
                return Outcome::refused(anchor, ProblemCode::BlockedReshape, &message);
            }
        }
        let entry = ParsedEntry {
            plate: lowest_plate(group),
            display_name: first.team_name.clone(),
            kind: EntryType::Team,
            riders: group.iter().map(|row| row.rider.clone()).collect(),
            notes: pooled_notes(group),
        };
        let size = team_size_outcome(&entry, self.roster, anchor);
        let mut conflicts = size.conflicts;
        if conflicts.is_empty() {
            conflicts.extend(pooled_structural_conflicts(
                group,
                &self.index,
                self.roster.status(),
            ));
        }
        Outcome {
            anchor,
            entry: Some(entry),
            conflicts,
            warnings: size.warnings,
        }
    }

    /// Keep the outcome's entry and findings.
    fn push(&mut self, outcome: Outcome) {
        if let Some(entry) = outcome.entry {
            self.assembly.entries.push(entry);
        }
        self.assembly.conflicts.extend(outcome.conflicts);
        self.assembly.warnings.extend(outcome.warnings);
    }
}

/// Map every roster rider plate to the entry that claims it.
fn pooled_owner_index(roster: &Roster) -> BTreeMap<&str, &Entry> {
    let mut index = BTreeMap::new();
    for entry in roster.entries() {
        for rider in &entry.riders {
            if let Some(plate) = rider.plate.as_deref() {
                index.insert(plate, entry);
            }
        }
    }
    index
}

/// Return the conflict when a pooled solo row must leave a team (spec S1).
fn pooled_solo_problem(
    index: &BTreeMap<&str, &Entry>,
    plate: &str,
    status: RideStatus,
) -> Option<String> {
    let owner = index.get(plate)?;
    if owner.kind != EntryType::Team {
        return None;
    }
    if can_edit_structure(status) {
        return None;
    }
    Some(format!(
        "converting a team member to a solo entry requires DRAFT (ride is {})",
        status.as_str()
    ))
}

/// Return the existing team that most of *rows* already belong to.
fn pooled_team_target<'a>(
    index: &BTreeMap<&str, &'a Entry>,
    rows: &[PooledRow],
) -> Option<&'a Entry> {
    let mut counts: Vec<(&'a Entry, usize)> = Vec::new();
    for plate in rows.iter().filter_map(|row| row.rider.plate.as_deref()) {
        let Some(owner) = index.get(plate) else {
            continue;
        };
        if owner.kind != EntryType::Team {
            continue;
        }
        match counts.iter_mut().find(|(entry, _)| entry.key == owner.key) {
            Some((_, count)) => *count += 1,
            None => counts.push((owner, 1)),
        }
    }
    let mut best: Option<(&'a Entry, usize)> = None;
    for (entry, count) in counts {
        if best.is_none_or(|(_, best_count)| count > best_count) {
            best = Some((entry, count));
        }
    }
    best.map(|(entry, _)| entry)
}

/// Return every membership reshape that a started pooled ride refuses.
fn pooled_structural_conflicts(
    rows: &[PooledRow],
    index: &BTreeMap<&str, &Entry>,
    status: RideStatus,
) -> Vec<ImportProblem> {
    if can_edit_structure(status) {
        return Vec::new();
    }
    let target = pooled_team_target(index, rows);
    let mut conflicts = Vec::new();
    for row in rows {
        let owner = row
            .rider
            .plate
            .as_deref()
            .and_then(|plate| index.get(plate));
        match owner {
            Some(owner) if target.is_some_and(|entry| entry.key == owner.key) => {}
            Some(owner) if owner.kind == EntryType::Team => conflicts.push(problem(
                row.row,
                ProblemCode::BlockedReshape,
                &membership_message(status),
            )),
            Some(_) => conflicts.push(problem(
                row.row,
                ProblemCode::BlockedReshape,
                &solo_to_team_message(status),
            )),
            None if target.is_some() => conflicts.push(problem(
                row.row,
                ProblemCode::BlockedReshape,
                &membership_message(status),
            )),
            None => {}
        }
    }
    conflicts
}

/// Return the conflict text for a team-to-team move that needs DRAFT.
fn membership_message(status: RideStatus) -> String {
    format!(
        "a team membership change requires DRAFT (ride is {})",
        status.as_str()
    )
}

/// Return the conflict text for a solo rider joining a team that needs DRAFT.
fn solo_to_team_message(status: RideStatus) -> String {
    format!(
        "converting a solo rider into a team member requires DRAFT (ride is {})",
        status.as_str()
    )
}

// --------------------------------------------------------------- warnings

/// Return one warning per rider name that repeats.
fn duplicate_rider_warnings(rows: &[DataRow]) -> Vec<ImportProblem> {
    let mut first: BTreeMap<String, String> = BTreeMap::new();
    let mut warned: BTreeSet<String> = BTreeSet::new();
    let mut warnings = Vec::new();
    for row in rows {
        let key = rider_name_key(&row.first_name, &row.last_name);
        if warned.contains(&key) {
            continue;
        }
        match first.get(&key) {
            Some(full_name) => {
                warnings.push(problem(
                    row.row,
                    ProblemCode::DuplicateRiderName,
                    &format!("duplicate rider name {full_name}"),
                ));
                warned.insert(key);
            }
            None => {
                first.insert(key, row.full_name());
            }
        }
    }
    warnings
}

/// Return one warning per pair of near-duplicate team names.
fn near_duplicate_team_warnings(
    rows: &[DataRow],
    converted: &BTreeSet<String>,
) -> Vec<ImportProblem> {
    let mut first_row: BTreeMap<&str, usize> = BTreeMap::new();
    let mut order: Vec<&str> = Vec::new();
    for row in rows.iter().filter(|row| row.is_team) {
        let key = row.team_key.as_str();
        if converted.contains(key) || first_row.contains_key(key) {
            continue;
        }
        first_row.insert(key, row.row);
        order.push(key);
    }
    let mut fuzzy_first: Vec<(&str, String)> = Vec::new();
    let mut warnings = Vec::new();
    for key in order {
        let fuzzy = team_name_key(key);
        match fuzzy_first.iter().find(|(_, earlier)| *earlier == fuzzy) {
            Some((earlier, _)) => {
                let row = first_row.get(key).copied().unwrap_or_default();
                warnings.push(problem(
                    row,
                    ProblemCode::NearDuplicateTeamName,
                    &format!("possible duplicate team name: \"{earlier}\" and \"{key}\""),
                ));
            }
            None => fuzzy_first.push((key, fuzzy)),
        }
    }
    warnings
}

// ---------------------------------------------------------------- helpers

/// Return the trimmed non-blank name parts joined by one space.
fn full_name(first: &str, last: &str) -> String {
    [first, last]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Return a parsed rider's full display name.
fn rider_full_name(rider: &ParsedRider) -> String {
    full_name(&rider.first_name, &rider.last_name)
}

/// Build the parsed rider of *row*, with *plate* when the model gives one.
fn parsed_rider(row: &DataRow, plate: Option<&str>) -> ParsedRider {
    ParsedRider {
        first_name: row.first_name.clone(),
        last_name: row.last_name.clone(),
        plate: plate.map(str::to_owned),
        sex: row.sex.clone(),
    }
}

/// Build one solo entry from *row*.
fn solo_entry(row: &DataRow, plate: &str, notes: &str) -> ParsedEntry {
    solo_entry_from_rider(&parsed_rider(row, None), plate, notes)
}

/// Build one solo entry around *rider*.
fn solo_entry_from_rider(rider: &ParsedRider, plate: &str, notes: &str) -> ParsedEntry {
    ParsedEntry {
        plate: plate.to_owned(),
        display_name: rider_full_name(rider),
        kind: EntryType::Solo,
        riders: vec![rider.clone()],
        notes: notes.to_owned(),
    }
}

/// Join a team group's non-blank notes with `"; "`.
fn grouped_notes(group: &[&DataRow]) -> String {
    let notes: Vec<&str> = group.iter().map(|row| row.notes.as_str()).collect();
    normalize::join_notes(&notes)
}

/// Join a pooled team group's non-blank notes with `"; "`.
fn pooled_notes(group: &[PooledRow]) -> String {
    let notes: Vec<&str> = group.iter().map(|row| row.notes.as_str()).collect();
    normalize::join_notes(&notes)
}

#[cfg(test)]
mod tests;
