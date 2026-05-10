use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, Parser)]
#[command(
    name = "profile",
    about = "Create, validate, and freeze column-scoping profiles",
    disable_help_subcommand = true,
    version
)]
pub struct Cli {
    /// Wrap output in the unified JSON envelope
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress witness ledger recording
    #[arg(long, global = true)]
    pub no_witness: bool,

    /// Print operator.json and exit 0 without positional args
    #[arg(long, global = true)]
    pub describe: bool,

    /// Print profile JSON Schema and exit 0
    #[arg(long, global = true)]
    pub schema: bool,

    /// Show raw data values in output (default: redacted for zero-retention safety)
    #[arg(long, global = true)]
    pub explicit: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Inspect the profile CLI contract without reading profile or dataset files
    Doctor(DoctorArgs),
    /// Create draft profiles from templates or dataset headers
    Draft(DraftArgs),
    /// Validate a profile against the schema
    Validate(ValidateArgs),
    /// Validate a profile and check column presence against a dataset
    Lint(LintArgs),
    /// Apply witnessed pre-parse slicing to a CSV-like dataset
    Slice(SliceArgs),
    /// Show structural statistics for a dataset
    Stats(StatsArgs),
    /// Rank candidate key columns by uniqueness
    SuggestKey(SuggestKeyArgs),
    /// Freeze a draft into an immutable, content-addressed profile
    Freeze(FreezeArgs),
    /// List available frozen profiles
    List(ListArgs),
    /// Show a resolved profile by ID
    Show(ShowArgs),
    /// Diff two profile versions
    Diff(DiffArgs),
    /// Publish a frozen profile to data-fabric (deferred in v0.1)
    Push(PushArgs),
    /// Fetch a frozen profile by ID from data-fabric (deferred in v0.1)
    Pull(PullArgs),
    /// Query the witness ledger
    Witness(WitnessArgs),
}

#[derive(Debug, Clone, Args, Default)]
pub struct DoctorArgs {
    /// Emit a machine-readable triage report for headless agents
    #[arg(long)]
    pub robot_triage: bool,

    #[command(subcommand)]
    pub command: Option<DoctorCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum DoctorCommand {
    /// Report read-only health and boundary checks
    Health,
    /// Report the supported doctor contract and domain boundaries
    Capabilities,
    /// Print concise usage guidance for headless agents
    RobotDocs,
}

#[derive(Debug, Clone, Args)]
pub struct DraftArgs {
    #[command(subcommand)]
    pub command: DraftCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum DraftCommand {
    /// Create a blank draft profile template
    New(DraftNewArgs),
    /// Create a draft profile from a dataset header
    Init(DraftInitArgs),
}

#[derive(Debug, Clone, ValueEnum)]
pub enum DatasetFormat {
    Csv,
}

#[derive(Debug, Clone, Args)]
pub struct DraftNewArgs {
    /// Dataset format for the profile template
    #[arg(long, value_enum, default_value_t = DatasetFormat::Csv)]
    pub format: DatasetFormat,

    /// Output path for the draft profile YAML
    #[arg(long)]
    pub out: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct DraftInitArgs {
    /// Path to the dataset to read headers from
    pub dataset: PathBuf,

    /// Output path for the draft profile YAML
    #[arg(long)]
    pub out: PathBuf,

    /// Dataset format
    #[arg(long, value_enum, default_value_t = DatasetFormat::Csv)]
    pub format: DatasetFormat,

    /// Key column name, or "auto" for automatic detection
    #[arg(long)]
    pub key: Option<String>,

    /// Canon registry directory used to normalize headers to canonical column IDs
    #[arg(long)]
    pub column_registry: Option<PathBuf>,

    /// JSON output from `fingerprint peek --suggest` used to seed pre_parse directives
    #[arg(long = "from-peek")]
    pub from_peek: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct ValidateArgs {
    /// Path to the profile YAML to validate
    pub file: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct LintArgs {
    /// Path to the profile YAML to lint
    pub profile: PathBuf,

    /// Path to the dataset to check columns against
    #[arg(long)]
    pub against: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct SliceArgs {
    /// Path to the dataset to slice
    pub file: PathBuf,

    /// Frozen or draft profile ID/path with pre_parse directives
    #[arg(long)]
    pub profile: Option<String>,

    /// Explicit profile YAML path with pre_parse directives
    #[arg(long = "profile-path")]
    pub profile_path: Option<PathBuf>,

    /// Slice mode for ad-hoc directives
    #[arg(long, value_enum)]
    pub mode: Option<SliceModeArg>,

    /// Number of leading rows to skip before the header
    #[arg(long = "skip-rows")]
    pub skip_rows: Option<usize>,

    /// 1-indexed physical row containing the header
    #[arg(long = "header-at-row")]
    pub header_at_row: Option<usize>,

    /// Comma-separated 1-indexed physical header rows for multi-row headers
    #[arg(long = "header-rows")]
    pub header_rows: Option<String>,

    /// Header merge strategy for multi-row headers
    #[arg(long = "header-merge", value_enum)]
    pub header_merge: Option<HeaderMergeStrategyArg>,

    /// Separator used when concatenating header levels
    #[arg(long = "header-merge-sep", default_value = ".")]
    pub header_merge_sep: String,

    /// Comma-separated 1-indexed physical unit rows to capture in the manifest
    #[arg(long = "unit-rows")]
    pub unit_rows: Option<String>,

    /// 1-indexed physical row where data starts
    #[arg(long = "data-starts-at")]
    pub data_starts_at: Option<usize>,

    /// One-character delimiter override
    #[arg(long)]
    pub delimiter: Option<String>,

    /// Encoding label. Only utf-8 is supported.
    #[arg(long)]
    pub encoding: Option<String>,

    /// Output CSV path. If omitted in human mode, clean CSV is written to stdout.
    #[arg(long)]
    pub out: Option<PathBuf>,

    /// Optional JSON manifest path for preamble, units, and lineage metadata
    #[arg(long = "emit-manifest")]
    pub emit_manifest: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum SliceModeArg {
    PreambleSkip,
    MultiRowHeader,
    PreambleWithUnits,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum HeaderMergeStrategyArg {
    FfillConcat,
    ConcatOnly,
    FirstNonEmpty,
}

#[derive(Debug, Clone, Args)]
pub struct StatsArgs {
    /// Path to the dataset to analyze
    pub dataset: PathBuf,

    /// Scope stats to columns in this profile
    #[arg(long)]
    pub profile: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct SuggestKeyArgs {
    /// Path to the dataset to analyze
    pub dataset: PathBuf,

    /// Number of top candidates to return
    #[arg(long, default_value_t = 5)]
    pub top: usize,
}

#[derive(Debug, Clone, Args)]
pub struct FreezeArgs {
    /// Path to the draft profile YAML to freeze
    pub draft: PathBuf,

    /// Profile family name (e.g. "csv.loan_tape.core")
    #[arg(long)]
    pub family: String,

    /// Integer version number for the frozen profile
    #[arg(long)]
    pub version: u64,

    /// Output path for the frozen profile YAML
    #[arg(long)]
    pub out: PathBuf,
}

#[derive(Debug, Clone, Args, Default)]
pub struct ListArgs {}

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    /// Profile ID to resolve and display
    pub profile_id: String,
}

#[derive(Debug, Clone, Args)]
pub struct DiffArgs {
    /// First profile ID or path
    pub a: String,
    /// Second profile ID or path
    pub b: String,
}

#[derive(Debug, Clone, Args)]
pub struct PushArgs {
    /// Path to the frozen profile to publish
    pub file: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct PullArgs {
    /// Profile ID to fetch from data-fabric
    pub profile_id: String,

    /// Output path for the downloaded profile
    #[arg(long)]
    pub out: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct WitnessArgs {
    #[command(subcommand)]
    pub command: WitnessCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum WitnessCommand {
    /// Search witness records with optional limit
    Query(WitnessQueryArgs),
    /// Show the most recent witness records
    Last(WitnessLastArgs),
    /// Count total witness records
    Count(WitnessCountArgs),
}

#[derive(Debug, Clone, Args, Default)]
pub struct WitnessQueryArgs {
    /// Maximum number of records to return
    #[arg(long)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Args)]
pub struct WitnessLastArgs {
    /// Number of most recent records to show
    #[arg(long, default_value_t = 1)]
    pub count: usize,
}

#[derive(Debug, Clone, Args, Default)]
pub struct WitnessCountArgs {}
