#![forbid(unsafe_code)]

use clap::{Parser, error::ErrorKind};
use serde_json::Value;

use crate::cli::args::{Cli, Command, DraftArgs, DraftCommand, WitnessArgs, WitnessCommand};
use crate::cli::exit::{EXIT_REFUSAL, EXIT_SUCCESS};
use crate::output::json::CommandOutput;
use crate::refusal::RefusalPayload;

pub mod cli;
pub mod diff;
pub mod draft;
pub mod freeze;
pub mod lint;
pub mod network;
pub mod output;
pub mod refusal;
pub mod resolve;
pub mod schema;
pub mod stats;
pub mod witness;

type HandlerResult = Result<CommandOutput, RefusalPayload>;

pub fn run() -> u8 {
    if let Some(display_mode) = detect_display_mode(std::env::args_os()) {
        return match display_mode {
            DisplayMode::Describe { json_output } => handle_describe(json_output),
            DisplayMode::Schema { json_output } => handle_schema(json_output),
        };
    }

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let kind = err.kind();
            if kind == ErrorKind::DisplayHelp || kind == ErrorKind::DisplayVersion {
                print!("{err}");
                return EXIT_SUCCESS;
            }

            eprint!("{err}");
            return EXIT_REFUSAL;
        }
    };

    if cli.describe {
        return handle_describe(cli.json);
    }

    if cli.schema {
        return handle_schema(cli.json);
    }

    let Some(command) = &cli.command else {
        eprintln!("No subcommand provided.");
        return EXIT_REFUSAL;
    };

    let result = dispatch(command, cli.no_witness, cli.explicit);
    let subcommand = command_name(command);

    if cli.json {
        output::json::emit(subcommand, result)
    } else {
        output::human::emit(subcommand, result)
    }
}

fn dispatch(command: &Command, no_witness: bool, explicit: bool) -> HandlerResult {
    match command {
        Command::Draft(DraftArgs { command }) => match command {
            DraftCommand::New(args) => {
                draft::new::run(args, no_witness).map(CommandOutput::success)
            }
            DraftCommand::Init(args) => {
                draft::init::run(args, no_witness).map(CommandOutput::success)
            }
        },
        Command::Validate(args) => lint::validate::run(args, no_witness),
        Command::Lint(args) => lint::lint::run(args, no_witness),
        Command::Stats(args) => stats::stats::run(args, no_witness, explicit),
        Command::SuggestKey(args) => stats::suggest_key::run(args, no_witness),
        Command::Freeze(args) => freeze::freeze::run(args, no_witness),
        Command::List(args) => resolve::list::run(args, no_witness).map(CommandOutput::success),
        Command::Show(args) => resolve::show::run(args, no_witness),
        Command::Diff(args) => diff::diff::run(args, no_witness).map(CommandOutput::success),
        Command::Push(args) => network::push::run(args, no_witness),
        Command::Pull(args) => network::pull::run(args, no_witness).map(CommandOutput::success),
        Command::Witness(WitnessArgs { command }) => match command {
            WitnessCommand::Query(args) => {
                witness::query::run_query(args).map(CommandOutput::success)
            }
            WitnessCommand::Last(args) => {
                witness::query::run_last(args).map(CommandOutput::success)
            }
            WitnessCommand::Count(args) => {
                witness::query::run_count(args).map(CommandOutput::success)
            }
        },
    }
}

fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Draft(DraftArgs { command }) => match command {
            DraftCommand::New(_) => "draft new",
            DraftCommand::Init(_) => "draft init",
        },
        Command::Validate(_) => "validate",
        Command::Lint(_) => "lint",
        Command::Stats(_) => "stats",
        Command::SuggestKey(_) => "suggest-key",
        Command::Freeze(_) => "freeze",
        Command::List(_) => "list",
        Command::Show(_) => "show",
        Command::Diff(_) => "diff",
        Command::Push(_) => "push",
        Command::Pull(_) => "pull",
        Command::Witness(WitnessArgs { command }) => match command {
            WitnessCommand::Query(_) => "witness query",
            WitnessCommand::Last(_) => "witness last",
            WitnessCommand::Count(_) => "witness count",
        },
    }
}

fn handle_describe(json_output: bool) -> u8 {
    const OPERATOR_JSON: &str = include_str!("../operator.json");

    if json_output {
        match serde_json::from_str::<Value>(OPERATOR_JSON) {
            Ok(operator_data) => {
                println!(
                    r#"{{"version":"profile.v0","outcome":"SUCCESS","exit_code":0,"subcommand":"describe","result":{},"profile_ref":null,"witness_id":null}}"#,
                    operator_data
                );
            }
            Err(_) => {
                println!(
                    r#"{{"version":"profile.v0","outcome":"REFUSAL","exit_code":2,"subcommand":"describe","result":{{"code":"E_IO","message":"Invalid operator.json format"}},"profile_ref":null,"witness_id":null}}"#
                );
                return EXIT_REFUSAL;
            }
        }
    } else {
        print!("{}", OPERATOR_JSON);
    }
    EXIT_SUCCESS
}

enum DisplayMode {
    Describe { json_output: bool },
    Schema { json_output: bool },
}

fn detect_display_mode<I, T>(args: I) -> Option<DisplayMode>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString>,
{
    let args = args
        .into_iter()
        .map(Into::into)
        .collect::<Vec<std::ffi::OsString>>();
    let json_output = args.iter().skip(1).any(|arg| arg == "--json");

    if args.iter().skip(1).any(|arg| arg == "--describe") {
        Some(DisplayMode::Describe { json_output })
    } else if args.iter().skip(1).any(|arg| arg == "--schema") {
        Some(DisplayMode::Schema { json_output })
    } else {
        None
    }
}

fn handle_schema(json_output: bool) -> u8 {
    let schema = output::generate_profile_schema();

    if json_output {
        let envelope = serde_json::json!({
            "version": "profile.v0",
            "outcome": "SUCCESS",
            "exit_code": 0,
            "subcommand": "schema",
            "result": schema,
            "profile_ref": null,
            "witness_id": null
        });
        println!("{}", envelope);
    } else {
        println!("{}", serde_json::to_string_pretty(&schema).unwrap());
    }

    EXIT_SUCCESS
}
