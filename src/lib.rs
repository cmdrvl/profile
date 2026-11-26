#![forbid(unsafe_code)]
#![recursion_limit = "512"]

use clap::{Parser, error::ErrorKind};
use serde_json::Value;

use crate::cli::args::{
    Cli, Command, DoctorArgs, DoctorCommand, DraftArgs, DraftCommand, WitnessArgs, WitnessCommand,
};
use crate::cli::exit::{EXIT_REFUSAL, EXIT_SUCCESS};
use crate::output::json::CommandOutput;
use crate::refusal::RefusalPayload;

pub mod cli;
pub mod diff;
pub mod discovery;
pub mod doctor;
pub mod draft;
pub mod evolve;
pub mod freeze;
pub mod inspect;
pub mod lint;
pub mod network;
pub mod output;
pub(crate) mod paths;
pub mod projection;
pub mod redaction;
pub mod refusal;
pub mod resolve;
pub mod schema;
pub mod slice;
pub mod stats;
pub mod tabular_input;
pub mod witness;

type HandlerResult = Result<CommandOutput, RefusalPayload>;

pub fn run() -> u8 {
    if let Some(display_mode) = detect_display_mode(std::env::args_os()) {
        return match display_mode {
            DisplayMode::Describe => handle_describe(),
            DisplayMode::Schema {
                json_output,
                target,
            } => handle_schema(json_output, target),
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
        return handle_describe();
    }

    if cli.schema {
        return handle_schema(cli.json, SchemaTarget::Profile);
    }

    if cli.robot_triage {
        return doctor::handle_command(None, true, true);
    }

    // Doctor and capabilities are machine-facing discovery surfaces. Keep
    // their JSON unwrapped so agents receive the canonical contract/report.
    match &cli.command {
        Some(Command::Capabilities) => return doctor::emit_capabilities(cli.json),
        Some(Command::Doctor(args)) => return doctor::handle_command(Some(args), false, cli.json),
        _ => {}
    }

    let Some(command) = &cli.command else {
        eprintln!("No subcommand provided.");
        return EXIT_REFUSAL;
    };

    if is_doctor_fix_request(command) {
        return doctor::emit_fix_unavailable();
    }

    // A set-but-unusable PROFILE_REDACTION_KEY refuses for every command that prints dataset
    // text, whether or not --explicit is given: never a silent fallback.
    let result = precheck_redaction_key(command)
        .and_then(|()| dispatch(command, cli.no_witness, cli.explicit, cli.json))
        .map_err(|payload| refusal::finish::finish(command, cli.explicit, payload));
    let subcommand = command_name(command);

    if cli.json {
        output::json::emit(subcommand, result)
    } else {
        output::human::emit(subcommand, result)
    }
}

fn precheck_redaction_key(command: &Command) -> Result<(), RefusalPayload> {
    match command {
        Command::Slice(_)
        | Command::Stats(_)
        | Command::Inspect(_)
        | Command::SuggestKey(_)
        | Command::Evolve(_) => redaction::RedactionKey::from_env()
            .map(|_| ())
            .map_err(RefusalPayload::redaction_key_invalid),
        _ => Ok(()),
    }
}

fn dispatch(
    command: &Command,
    no_witness: bool,
    explicit: bool,
    json_output: bool,
) -> HandlerResult {
    match command {
        Command::Doctor(args) => doctor::run(args).map(CommandOutput::success),
        Command::Capabilities => Ok(CommandOutput::success(doctor::capabilities_report())),
        Command::RobotDocs(_) => Ok(CommandOutput::success(doctor::robot_docs())),
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
        Command::Evolve(args) => evolve::run(args, no_witness, explicit),
        Command::Slice(args) => slice::run(args, no_witness, explicit, json_output),
        Command::Stats(args) => stats::stats::run(args, no_witness, explicit),
        Command::Inspect(args) => inspect::run(args, explicit),
        Command::SuggestKey(args) => stats::suggest_key::run(args, no_witness, explicit),
        Command::Freeze(args) => freeze::freeze::run(args, no_witness),
        Command::List(args) => resolve::list::run(args, no_witness).map(CommandOutput::success),
        Command::Show(args) => resolve::show::run(args, no_witness),
        Command::Diff(args) => diff::diff::run(args, no_witness).map(CommandOutput::success),
        Command::EmitDiscovery(args) => discovery::emit::run(args),
        Command::EmitVerifyStarter(args) => projection::emit_verify_starter(args, no_witness),
        Command::EmitDdl(args) => projection::emit_ddl(args, no_witness),
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
        Command::Capabilities => "capabilities",
        Command::RobotDocs(_) => "robot-docs",
        Command::Doctor(DoctorArgs {
            robot_triage: true, ..
        }) => "doctor triage",
        Command::Doctor(DoctorArgs { fix: true, .. }) => "doctor fix",
        Command::Doctor(DoctorArgs {
            command: Some(DoctorCommand::Capabilities),
            ..
        }) => "doctor capabilities",
        Command::Doctor(DoctorArgs {
            command: Some(DoctorCommand::Triage(_)),
            ..
        }) => "doctor triage",
        Command::Doctor(DoctorArgs {
            command: Some(DoctorCommand::RobotDocs),
            ..
        }) => "doctor robot-docs",
        Command::Doctor(DoctorArgs {
            command: Some(DoctorCommand::Fix),
            ..
        }) => "doctor fix",
        Command::Doctor(_) => "doctor health",
        Command::Draft(DraftArgs { command }) => match command {
            DraftCommand::New(_) => "draft new",
            DraftCommand::Init(_) => "draft init",
        },
        Command::Validate(_) => "validate",
        Command::Lint(_) => "lint",
        Command::Evolve(_) => "evolve",
        Command::Slice(_) => "slice",
        Command::Stats(_) => "stats",
        Command::Inspect(_) => "inspect",
        Command::SuggestKey(_) => "suggest-key",
        Command::Freeze(_) => "freeze",
        Command::List(_) => "list",
        Command::Show(_) => "show",
        Command::Diff(_) => "diff",
        Command::EmitDiscovery(_) => "emit-discovery",
        Command::EmitVerifyStarter(_) => "emit-verify-starter",
        Command::EmitDdl(_) => "emit-ddl",
        Command::Push(_) => "push",
        Command::Pull(_) => "pull",
        Command::Witness(WitnessArgs { command }) => match command {
            WitnessCommand::Query(_) => "witness query",
            WitnessCommand::Last(_) => "witness last",
            WitnessCommand::Count(_) => "witness count",
        },
    }
}

fn is_doctor_fix_request(command: &Command) -> bool {
    matches!(
        command,
        Command::Doctor(DoctorArgs { fix: true, .. })
            | Command::Doctor(DoctorArgs {
                command: Some(DoctorCommand::Fix),
                ..
            })
    )
}

fn handle_describe() -> u8 {
    const OPERATOR_JSON: &str = include_str!("../operator.json");

    if serde_json::from_str::<Value>(OPERATOR_JSON).is_err() {
        eprintln!("Invalid operator.json format");
        return EXIT_REFUSAL;
    }

    print!("{}", OPERATOR_JSON);
    EXIT_SUCCESS
}

enum DisplayMode {
    Describe,
    Schema {
        json_output: bool,
        target: SchemaTarget,
    },
}

enum SchemaTarget {
    Profile,
    Discovery,
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
        Some(DisplayMode::Describe)
    } else if args.iter().skip(1).any(|arg| arg == "--schema") {
        let target = if args.iter().skip(1).any(|arg| arg == "emit-discovery") {
            SchemaTarget::Discovery
        } else {
            SchemaTarget::Profile
        };
        Some(DisplayMode::Schema {
            json_output,
            target,
        })
    } else {
        None
    }
}

fn handle_schema(json_output: bool, target: SchemaTarget) -> u8 {
    let schema = match target {
        SchemaTarget::Profile => output::generate_profile_schema(),
        SchemaTarget::Discovery => output::generate_discovery_schema(),
    };

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
