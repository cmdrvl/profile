#![forbid(unsafe_code)]

use clap::{Parser, error::ErrorKind};
use serde_json::Value;

use crate::cli::args::{Cli, Command, DraftArgs, DraftCommand, WitnessArgs, WitnessCommand};
use crate::cli::exit::{EXIT_REFUSAL, EXIT_SUCCESS};
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

type HandlerResult = Result<Value, RefusalPayload>;

pub fn run() -> u8 {
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

    let result = dispatch(command, cli.no_witness);
    let subcommand = command_name(command);

    if cli.json {
        output::json::emit(subcommand, result)
    } else {
        output::human::emit(subcommand, result)
    }
}

fn dispatch(command: &Command, no_witness: bool) -> HandlerResult {
    match command {
        Command::Draft(DraftArgs { command }) => match command {
            DraftCommand::New(args) => draft::new::run(args, no_witness),
            DraftCommand::Init(args) => draft::init::run(args, no_witness),
        },
        Command::Validate(args) => lint::validate::run(args, no_witness),
        Command::Lint(args) => lint::lint::run(args, no_witness),
        Command::Stats(args) => stats::stats::run(args, no_witness),
        Command::SuggestKey(args) => stats::suggest_key::run(args, no_witness),
        Command::Freeze(args) => freeze::freeze::run(args, no_witness),
        Command::List(args) => resolve::list::run(args, no_witness),
        Command::Show(args) => resolve::show::run(args, no_witness),
        Command::Diff(args) => diff::diff::run(args, no_witness),
        Command::Push(args) => network::push::run(args, no_witness),
        Command::Pull(args) => network::pull::run(args, no_witness),
        Command::Witness(WitnessArgs { command }) => match command {
            WitnessCommand::Query(args) => witness::query::run_query(args),
            WitnessCommand::Last(args) => witness::query::run_last(args),
            WitnessCommand::Count(args) => witness::query::run_count(args),
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
    match std::fs::read_to_string("operator.json") {
        Ok(content) => {
            if json_output {
                // Parse the operator.json and embed it in the envelope
                match serde_json::from_str::<Value>(&content) {
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
                println!("{}", content);
            }
            EXIT_SUCCESS
        }
        Err(_) => {
            if json_output {
                println!(
                    r#"{{"version":"profile.v0","outcome":"REFUSAL","exit_code":2,"subcommand":"describe","result":{{"code":"E_IO","message":"Failed to read operator.json"}},"profile_ref":null,"witness_id":null}}"#
                );
            } else {
                eprintln!("Failed to read operator.json");
            }
            EXIT_REFUSAL
        }
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
