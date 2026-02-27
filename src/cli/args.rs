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
    #[arg(long, global = true)]
    pub json: bool,

    #[arg(long, global = true)]
    pub no_witness: bool,

    #[arg(long, global = true)]
    pub describe: bool,

    #[arg(long, global = true)]
    pub schema: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    Draft(DraftArgs),
    Validate(ValidateArgs),
    Lint(LintArgs),
    Stats(StatsArgs),
    SuggestKey(SuggestKeyArgs),
    Freeze(FreezeArgs),
    List(ListArgs),
    Show(ShowArgs),
    Diff(DiffArgs),
    Witness(WitnessArgs),
}

#[derive(Debug, Clone, Args)]
pub struct DraftArgs {
    #[command(subcommand)]
    pub command: DraftCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum DraftCommand {
    New(DraftNewArgs),
    Init(DraftInitArgs),
}

#[derive(Debug, Clone, ValueEnum)]
pub enum DatasetFormat {
    Csv,
}

#[derive(Debug, Clone, Args)]
pub struct DraftNewArgs {
    #[arg(long, value_enum, default_value_t = DatasetFormat::Csv)]
    pub format: DatasetFormat,

    #[arg(long)]
    pub out: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct DraftInitArgs {
    pub dataset: PathBuf,

    #[arg(long)]
    pub out: PathBuf,

    #[arg(long, value_enum, default_value_t = DatasetFormat::Csv)]
    pub format: DatasetFormat,

    #[arg(long)]
    pub key: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct ValidateArgs {
    pub file: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct LintArgs {
    pub profile: PathBuf,

    #[arg(long)]
    pub against: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct StatsArgs {
    pub dataset: PathBuf,

    #[arg(long)]
    pub profile: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct SuggestKeyArgs {
    pub dataset: PathBuf,

    #[arg(long, default_value_t = 5)]
    pub top: usize,
}

#[derive(Debug, Clone, Args)]
pub struct FreezeArgs {
    pub draft: PathBuf,

    #[arg(long)]
    pub family: String,

    #[arg(long)]
    pub version: u64,

    #[arg(long)]
    pub out: PathBuf,
}

#[derive(Debug, Clone, Args, Default)]
pub struct ListArgs {}

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    pub profile_id: String,
}

#[derive(Debug, Clone, Args)]
pub struct DiffArgs {
    pub a: String,
    pub b: String,
}

#[derive(Debug, Clone, Args)]
pub struct WitnessArgs {
    #[command(subcommand)]
    pub command: WitnessCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum WitnessCommand {
    Query(WitnessQueryArgs),
    Last(WitnessLastArgs),
    Count(WitnessCountArgs),
}

#[derive(Debug, Clone, Args, Default)]
pub struct WitnessQueryArgs {
    #[arg(long)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Args)]
pub struct WitnessLastArgs {
    #[arg(long, default_value_t = 1)]
    pub count: usize,
}

#[derive(Debug, Clone, Args, Default)]
pub struct WitnessCountArgs {}
