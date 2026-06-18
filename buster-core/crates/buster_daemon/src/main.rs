use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::time::Duration;

use buster_daemon::arena;
use buster_daemon::harness;
use buster_daemon::organism;
use buster_daemon::paper_acquisition;
use buster_daemon::paper_brief;
use buster_daemon::paperqa;
use buster_daemon::protocol_security;
use buster_daemon::security_research;
use buster_daemon::web::{serve, WebConfig};
use buster_daemon::web_search;
use buster_daemon::{BusterDaemon, DaemonConfig};
use buster_skills::{Skill, SkillWorkspace};
use buster_tools::{ToolActionResult, ToolWorkspace};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(command) = args.first().map(String::as_str) else {
        print_usage();
        return Ok(());
    };

    match command {
        "tick" => {
            let mut config = parse_options(&args[1..])?;
            config.max_ticks = Some(1);
            let mut daemon = BusterDaemon::new(config).map_err(|error| error.to_string())?;
            let record = daemon.tick(0).map_err(|error| error.to_string())?;
            print_record(&record);
            Ok(())
        }
        "run" => {
            let config = parse_options(&args[1..])?;
            let mut daemon = BusterDaemon::new(config).map_err(|error| error.to_string())?;
            let records = daemon.run().map_err(|error| error.to_string())?;
            for record in records {
                print_record(&record);
            }
            Ok(())
        }
        "web" => {
            let config = parse_web_options(&args[1..])?;
            serve(config).map_err(|error| error.to_string())
        }
        "arena" => run_arena_command(&args[1..]),
        "node" => run_node_command(&args[1..]),
        "harness" => run_harness_command(&args[1..]),
        "papers" => run_papers_command(&args[1..]),
        "paperqa" => run_paperqa_command(&args[1..]),
        "web-search" => run_web_search_command(&args[1..]),
        "security-research" => run_security_research_command(&args[1..]),
        "protocol-security" => run_protocol_security_command(&args[1..]),
        "skills" => run_skills_command(&args[1..]),
        "tools" => run_tools_command(&args[1..]),
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown command `{other}`")),
    }
}

fn parse_options(args: &[String]) -> Result<DaemonConfig, String> {
    let mut config = DaemonConfig::new(env::current_dir().map_err(|error| error.to_string())?);
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                config.root = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "--root requires a path".to_string())?,
                );
            }
            "--ticks" => {
                index += 1;
                let ticks = args
                    .get(index)
                    .ok_or_else(|| "--ticks requires a positive integer".to_string())?
                    .parse()
                    .map_err(|_| "--ticks must be a positive integer".to_string())?;
                config.max_ticks = Some(ticks);
            }
            "--forever" => {
                config.max_ticks = None;
            }
            "--interval-ms" => {
                index += 1;
                let interval_ms = args
                    .get(index)
                    .ok_or_else(|| "--interval-ms requires a positive integer".to_string())?
                    .parse()
                    .map_err(|_| "--interval-ms must be a positive integer".to_string())?;
                config.tick_interval = Duration::from_millis(interval_ms);
            }
            "--max-context-bytes" => {
                index += 1;
                config.max_context_bytes = args
                    .get(index)
                    .ok_or_else(|| "--max-context-bytes requires a positive integer".to_string())?
                    .parse()
                    .map_err(|_| "--max-context-bytes must be a positive integer".to_string())?;
            }
            "--no-body-reports" => {
                config.write_body_reports = false;
            }
            "--execute-actions" => {
                config.execute_actions = true;
            }
            other => return Err(format!("unknown option `{other}`")),
        }
        index += 1;
    }
    Ok(config)
}

fn parse_web_options(args: &[String]) -> Result<WebConfig, String> {
    let mut web = WebConfig::new(env::current_dir().map_err(|error| error.to_string())?);
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                web.daemon.root = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "--root requires a path".to_string())?,
                );
            }
            "--port" => {
                index += 1;
                web.port = args
                    .get(index)
                    .ok_or_else(|| "--port requires a port".to_string())?
                    .parse()
                    .map_err(|_| "--port must be a number".to_string())?;
            }
            "--host" => {
                index += 1;
                web.host = args
                    .get(index)
                    .ok_or_else(|| "--host requires an address".to_string())?
                    .to_string();
            }
            "--interval-ms" => {
                index += 1;
                let interval_ms = args
                    .get(index)
                    .ok_or_else(|| "--interval-ms requires a positive integer".to_string())?
                    .parse()
                    .map_err(|_| "--interval-ms must be a positive integer".to_string())?;
                web.daemon.tick_interval = Duration::from_millis(interval_ms);
            }
            "--max-context-bytes" => {
                index += 1;
                web.daemon.max_context_bytes = args
                    .get(index)
                    .ok_or_else(|| "--max-context-bytes requires a positive integer".to_string())?
                    .parse()
                    .map_err(|_| "--max-context-bytes must be a positive integer".to_string())?;
            }
            "--execute-actions" => {
                web.daemon.execute_actions = true;
            }
            "--no-open" => {
                web.open_browser = false;
            }
            "--no-body-reports" => {
                web.daemon.write_body_reports = false;
            }
            other => return Err(format!("unknown web option `{other}`")),
        }
        index += 1;
    }
    Ok(web)
}

fn run_node_command(args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err(
            "node requires a subcommand: init | heartbeat | verify | snapshot | witness | import-receipt | registry"
                .to_string(),
        );
    };
    match subcommand {
        "init" => {
            let root = parse_root_arg(&args[1..])?;
            let identity = organism::ensure_node(&root).map_err(|error| error.to_string())?;
            let manifest = organism::write_body_manifest(&root, &identity)
                .map_err(|error| error.to_string())?;
            organism::append_signed_event(
                &root,
                "node_initialized",
                serde_json::json!({
                    "node_id": identity.node_id,
                    "manifest_version": manifest.manifest_version,
                }),
            )
            .map_err(|error| error.to_string())?;
            println!("node_id={}", identity.node_id);
            for path in organism::node_paths(&root) {
                println!("wrote={}", path.display());
            }
            Ok(())
        }
        "heartbeat" => {
            let root = parse_root_arg(&args[1..])?;
            let event = organism::append_heartbeat(&root).map_err(|error| error.to_string())?;
            println!(
                "heartbeat node={} event={} hash={}",
                event.node_id, event.event_id, event.event_hash
            );
            Ok(())
        }
        "verify" => {
            let root = parse_root_arg(&args[1..])?;
            let report = organism::verify_event_log(&root).map_err(|error| error.to_string())?;
            println!(
                "valid={} checked_events={} error={}",
                report.valid,
                report.checked_events,
                report.error.unwrap_or_else(|| "none".to_string())
            );
            Ok(())
        }
        "snapshot" => {
            let (root, out) = parse_root_and_optional_file_arg(&args[1..], "--out")?;
            let snapshot = organism::export_snapshot(&root).map_err(|error| error.to_string())?;
            write_json_output(out, &snapshot)
        }
        "witness" => {
            let (root, snapshot_path) = parse_root_and_required_file_arg(&args[1..], "--snapshot")?;
            let snapshot_path =
                snapshot_path.ok_or_else(|| "node witness requires --snapshot PATH".to_string())?;
            let snapshot_text =
                fs::read_to_string(&snapshot_path).map_err(|error| error.to_string())?;
            let snapshot: organism::NodeSnapshot =
                serde_json::from_str(&snapshot_text).map_err(|error| error.to_string())?;
            let receipt =
                organism::witness_snapshot(&root, &snapshot).map_err(|error| error.to_string())?;
            write_json_output(None, &receipt)
        }
        "import-receipt" => {
            let (root, receipt_path) = parse_root_and_required_file_arg(&args[1..], "--receipt")?;
            let receipt_path = receipt_path
                .ok_or_else(|| "node import-receipt requires --receipt PATH".to_string())?;
            let receipt_text =
                fs::read_to_string(&receipt_path).map_err(|error| error.to_string())?;
            let receipt: organism::WitnessReceipt =
                serde_json::from_str(&receipt_text).map_err(|error| error.to_string())?;
            organism::import_witness_receipt(&root, &receipt).map_err(|error| error.to_string())?;
            println!(
                "imported receipt witness={} subject={} head={}",
                receipt.witness_node_id,
                receipt.subject_node_id,
                receipt.subject_chain_head.as_deref().unwrap_or("none")
            );
            Ok(())
        }
        "registry" => {
            let root = parse_root_arg(&args[1..])?;
            let registry = organism::read_known_nodes(&root).map_err(|error| error.to_string())?;
            write_json_output(None, &registry)
        }
        other => Err(format!("unknown node subcommand `{other}`")),
    }
}

fn run_harness_command(args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err("harness requires a subcommand: tail-contracts | tail-outcomes".to_string());
    };
    let (root, options) = parse_root_and_key_values(&args[1..])?;
    let max_lines = option_value(&options, "--lines")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10);
    let relative_path = match subcommand {
        "tail-contracts" => harness::CONTRACT_AUDIT_PATH,
        "tail-outcomes" => harness::OUTCOME_AUDIT_PATH,
        other => return Err(format!("unknown harness subcommand `{other}`")),
    };
    let lines =
        harness::tail_file(&root, relative_path, max_lines).map_err(|error| error.to_string())?;
    for line in lines {
        println!("{line}");
    }
    Ok(())
}

fn run_paperqa_command(args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err("paperqa requires a subcommand: doctor | ask | tail".to_string());
    };
    let (root, options) = parse_root_and_key_values(&args[1..])?;
    match subcommand {
        "doctor" => {
            let report = paperqa::doctor(&root).map_err(|error| error.to_string())?;
            write_json_output(None, &report)
        }
        "ask" => {
            let question = required_option(&options, "--question")?;
            let mut request = paperqa::PaperQaRequest::new(question);
            if let Some(task_id) = option_value(&options, "--task-id") {
                request.task_id = Some(task_id.to_string());
            }
            if let Some(settings) = option_value(&options, "--settings") {
                request.settings = Some(settings.to_string());
            }
            if let Some(index_name) = option_value(&options, "--index") {
                request.index_name = Some(index_name.to_string());
            }
            if let Some(llm) = option_value(&options, "--llm") {
                request.llm = Some(llm.to_string());
            }
            if let Some(domain) = option_value(&options, "--domain") {
                request.domain = parse_research_domain(domain)?;
            }
            let record = paperqa::ask(&root, request).map_err(|error| error.to_string())?;
            write_json_output(None, &record)
        }
        "tail" => {
            let max_lines = option_value(&options, "--lines")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(10);
            for line in paperqa::tail(&root, max_lines).map_err(|error| error.to_string())? {
                println!("{line}");
            }
            Ok(())
        }
        other => Err(format!("unknown paperqa subcommand `{other}`")),
    }
}

fn run_papers_command(args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err(
            "papers requires a subcommand: acquire | brief | tail | brief-tail".to_string(),
        );
    };
    let (root, options) = parse_root_and_key_values(&args[1..])?;
    match subcommand {
        "acquire" => {
            let question = required_option(&options, "--question")?;
            let mut request = paper_acquisition::PaperAcquisitionRequest::new(question);
            if let Some(task_id) = option_value(&options, "--task-id") {
                request.task_id = Some(task_id.to_string());
            }
            if let Some(domain) = option_value(&options, "--domain") {
                request.domain = parse_research_domain(domain)?;
            }
            if let Some(limit) = option_value(&options, "--limit") {
                request.limit = limit
                    .parse()
                    .map_err(|_| "--limit must be a positive integer".to_string())?;
            }
            let record = paper_acquisition::acquire_open_papers(&root, request)
                .map_err(|error| error.to_string())?;
            write_json_output(None, &record)
        }
        "brief" => {
            let question = required_option(&options, "--question")?;
            let mut request = paper_brief::PaperBriefRequest::new(question);
            if let Some(task_id) = option_value(&options, "--task-id") {
                request.task_id = Some(task_id.to_string());
            }
            if let Some(domain) = option_value(&options, "--domain") {
                request.domain = parse_research_domain(domain)?;
            }
            if let Some(max_papers) = option_value(&options, "--max-papers") {
                request.max_papers = max_papers
                    .parse()
                    .map_err(|_| "--max-papers must be a positive integer".to_string())?;
            }
            let record =
                paper_brief::brief_latest(&root, request).map_err(|error| error.to_string())?;
            write_json_output(None, &record)
        }
        "tail" => {
            let max_lines = option_value(&options, "--lines")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(10);
            for line in
                paper_acquisition::tail(&root, max_lines).map_err(|error| error.to_string())?
            {
                println!("{line}");
            }
            Ok(())
        }
        "brief-tail" => {
            let max_lines = option_value(&options, "--lines")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(10);
            for line in paper_brief::tail(&root, max_lines).map_err(|error| error.to_string())? {
                println!("{line}");
            }
            Ok(())
        }
        other => Err(format!("unknown papers subcommand `{other}`")),
    }
}

fn run_arena_command(args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err(
            "arena requires a subcommand: status | competitions | onboarding | register | fetch-skill | heartbeat | strategy | eval-start | eval-tick"
                .to_string(),
        );
    };
    let (root, options) = parse_root_and_key_values(&args[1..])?;
    match subcommand {
        "status" => {
            let report = arena::status(&root).map_err(|error| error.to_string())?;
            write_json_output(None, &report)
        }
        "competitions" => {
            let competitions = arena::ArenaClient::new()
                .list_active_competitions(&root)
                .map_err(|error| error.to_string())?;
            write_json_output(None, &competitions)
        }
        "onboarding" => {
            let preferred = option_value(&options, "--prefer");
            let draft =
                arena::onboarding_draft(&root, preferred).map_err(|error| error.to_string())?;
            write_json_output(None, &draft)
        }
        "register" => {
            let name = required_option(&options, "--name")?;
            let quote = required_option(&options, "--quote")?;
            let record = arena::ArenaClient::new()
                .register(&root, name, quote)
                .map_err(|error| error.to_string())?;
            write_json_output(None, &record)
        }
        "fetch-skill" => {
            let skill_file = if let Some(skill_file) = option_value(&options, "--skill-file") {
                skill_file.to_string()
            } else if let Some(preferred) = option_value(&options, "--prefer") {
                let competitions = arena::ArenaClient::new()
                    .list_active_competitions(&root)
                    .map_err(|error| error.to_string())?;
                let competition = arena::select_competition(&competitions, Some(preferred))
                    .ok_or_else(|| "no matching arena competition found".to_string())?;
                arena::skill_file_for_competition(&competition).to_string()
            } else {
                "/skills/arena.md".to_string()
            };
            let record = arena::ArenaClient::new()
                .fetch_skill(&root, &skill_file)
                .map_err(|error| error.to_string())?;
            write_json_output(None, &record)
        }
        "heartbeat" => {
            let force = option_value(&options, "--force")
                .map(|value| matches!(value, "1" | "true" | "yes" | "on"))
                .unwrap_or(false);
            let report = arena::heartbeat(&root, force).map_err(|error| error.to_string())?;
            write_json_output(None, &report)
        }
        "strategy" => {
            let preferred = option_value(&options, "--prefer");
            let competitions = arena::ArenaClient::new()
                .list_active_competitions(&root)
                .unwrap_or_default();
            let competition = arena::select_competition(&competitions, preferred);
            let state = arena::ensure_strategy_state(&root, competition.as_ref())
                .map_err(|error| error.to_string())?;
            write_json_output(None, &state)
        }
        "eval-start" => {
            let preferred = option_value(&options, "--prefer");
            let report = arena::eval_start(&root, preferred).map_err(|error| error.to_string())?;
            write_json_output(None, &report)
        }
        "eval-tick" => {
            let preferred = option_value(&options, "--prefer");
            let execute = option_value(&options, "--execute")
                .map(|value| matches!(value, "1" | "true" | "yes" | "on"))
                .unwrap_or(false);
            let report =
                arena::eval_tick(&root, preferred, execute).map_err(|error| error.to_string())?;
            write_json_output(None, &report)
        }
        other => Err(format!("unknown arena subcommand `{other}`")),
    }
}

fn run_web_search_command(args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err("web-search requires a subcommand: search | tail".to_string());
    };
    let (root, options) = parse_root_and_key_values(&args[1..])?;
    match subcommand {
        "search" => {
            let query = required_option(&options, "--query")?;
            let mut request = web_search::WebSearchRequest::new(query);
            if let Some(count) = option_value(&options, "--count") {
                request.count = count
                    .parse()
                    .map_err(|_| "--count must be a positive integer".to_string())?;
            }
            if let Some(region) = option_value(&options, "--region") {
                request.region = Some(region.to_string());
            }
            if let Some(safe_search) = option_value(&options, "--safe-search") {
                request.safe_search = web_search::SafeSearch::parse(safe_search)?;
            }
            if let Some(task_id) = option_value(&options, "--task-id") {
                request.task_id = Some(task_id.to_string());
            }

            let contract = harness::web_search_contract(buster_daemon::now_secs(), &request.query);
            let contract_id = contract.contract_id.clone();
            harness::append_contract(&root, &contract).map_err(|error| error.to_string())?;
            let bundle = web_search::WebSearchClient::new()
                .search(&root, request)
                .map_err(|error| error.to_string())?;
            let status = if bundle.errors.is_empty() {
                harness::HarnessStatus::Completed
            } else if bundle.results.is_empty() {
                harness::HarnessStatus::Failed
            } else {
                harness::HarnessStatus::Partial
            };
            let mut outcome = harness::HarnessOutcome::new(
                contract_id,
                status,
                format!(
                    "web search returned {} result(s), {} error(s)",
                    bundle.results.len(),
                    bundle.errors.len()
                ),
            )
            .with_output(
                "bundle",
                &bundle.bundle_path.display().to_string(),
                "web search bundle",
            )
            .with_output("audit", web_search::WEB_SEARCH_AUDIT, "web search audit");
            if !bundle.errors.is_empty() {
                outcome = outcome.with_error(
                    bundle
                        .errors
                        .iter()
                        .map(|error| error.message.clone())
                        .collect::<Vec<_>>()
                        .join(" | "),
                );
            }
            harness::append_outcome(&root, &outcome).map_err(|error| error.to_string())?;
            write_json_output(None, &bundle)
        }
        "tail" => {
            let max_lines = option_value(&options, "--lines")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(10);
            for line in web_search::tail(&root, max_lines).map_err(|error| error.to_string())? {
                println!("{line}");
            }
            Ok(())
        }
        other => Err(format!("unknown web-search subcommand `{other}`")),
    }
}

fn run_security_research_command(args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err(
            "security-research requires a subcommand: taskflows | audit-local | tail".to_string(),
        );
    };
    let (root, options) = parse_root_and_key_values(&args[1..])?;
    match subcommand {
        "taskflows" => {
            let taskflows = security_research::ensure_security_taskflows(&root)
                .map_err(|error| error.to_string())?;
            write_json_output(None, &taskflows)
        }
        "audit-local" => {
            let report = security_research::run_local_repository_audit(&root)
                .map_err(|error| error.to_string())?;
            write_json_output(None, &report)
        }
        "tail" => {
            let max_lines = option_value(&options, "--lines")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(10);
            print_tail(
                root.join(security_research::SECURITY_REPORT_AUDIT),
                max_lines,
            )
        }
        other => Err(format!("unknown security-research subcommand `{other}`")),
    }
}

fn run_protocol_security_command(args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err("protocol-security requires a subcommand: check | tail".to_string());
    };
    let (root, options) = parse_root_and_key_values(&args[1..])?;
    match subcommand {
        "check" => {
            let report = protocol_security::check_protocol_invariants(&root)
                .map_err(|error| error.to_string())?;
            write_json_output(None, &report)
        }
        "tail" => {
            let max_lines = option_value(&options, "--lines")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(10);
            print_tail(
                root.join(protocol_security::PROTOCOL_SECURITY_AUDIT),
                max_lines,
            )
        }
        other => Err(format!("unknown protocol-security subcommand `{other}`")),
    }
}

fn parse_research_domain(value: &str) -> Result<buster_value_model::ResearchDomain, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "theoreticalphysics" | "theoretical-physics" | "physics" => {
            Ok(buster_value_model::ResearchDomain::TheoreticalPhysics)
        }
        "quantumcomputing" | "quantum-computing" | "quantum" => {
            Ok(buster_value_model::ResearchDomain::QuantumComputing)
        }
        "nuclearfusion" | "nuclear-fusion" | "fusion" => {
            Ok(buster_value_model::ResearchDomain::NuclearFusion)
        }
        "lifescience"
        | "life-science"
        | "biology"
        | "drugdiscovery"
        | "drug-discovery"
        | "pharma"
        | "life-science-and-pharma" => Ok(buster_value_model::ResearchDomain::LifeScienceAndPharma),
        "genetics" | "gene" => Ok(buster_value_model::ResearchDomain::Genetics),
        "materialsscience" | "materials-science" | "materials" => {
            Ok(buster_value_model::ResearchDomain::MaterialsScience)
        }
        "braincomputerinterface" | "brain-computer-interface" | "bci" => {
            Ok(buster_value_model::ResearchDomain::BrainComputerInterface)
        }
        "aerospace" | "space" => Ok(buster_value_model::ResearchDomain::Aerospace),
        "robotics" | "robot" => Ok(buster_value_model::ResearchDomain::Robotics),
        "cybersecurity" | "security" => Ok(buster_value_model::ResearchDomain::Cybersecurity),
        "protocolsecurity" | "protocol-security" | "protocol" => {
            Ok(buster_value_model::ResearchDomain::ProtocolSecurity)
        }
        "other" => Ok(buster_value_model::ResearchDomain::Other),
        other => Err(format!("unknown research domain `{other}`")),
    }
}

fn run_skills_command(args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err(
            "skills requires a subcommand: scan | list | view | record-use | draft | install"
                .to_string(),
        );
    };
    let (root, options) = parse_root_and_key_values(&args[1..])?;
    let workspace = SkillWorkspace::new(&root);
    match subcommand {
        "scan" => {
            let registry = workspace
                .refresh_registry()
                .map_err(|error| error.to_string())?;
            write_json_output(None, &registry)
        }
        "list" => {
            let registry = workspace
                .read_registry()
                .or_else(|_| workspace.refresh_registry())
                .map_err(|error| error.to_string())?;
            write_json_output(None, &registry.entries)
        }
        "view" => {
            let name = required_option(&options, "--name")?;
            let view = workspace
                .view_skill(name)
                .map_err(|error| error.to_string())?;
            write_json_output(None, &view)
        }
        "record-use" => {
            let name = required_option(&options, "--name")?;
            let context = option_value(&options, "--context").unwrap_or("manual use");
            let outcome = option_value(&options, "--outcome").unwrap_or("not specified");
            let receipt = workspace
                .record_use(name, context, outcome)
                .map_err(|error| error.to_string())?;
            write_json_output(None, &receipt)
        }
        "draft" => {
            let name = required_option(&options, "--name")?;
            let trigger = required_option(&options, "--trigger")?;
            let purpose = required_option(&options, "--purpose")?;
            let procedure = required_option(&options, "--procedure")?;
            let validation = option_value(&options, "--validation")
                .unwrap_or("Confirm the result is useful, bounded, and does not contain secrets.");
            let evidence = option_values(&options, "--evidence");
            let skill = Skill::new(name, trigger, purpose, procedure, validation).with_disabled_when([
                "The task would modify SELF.md, GOVERNANCE.md, BODY.md, or VALUE_MODEL.md.",
                "The task requires secrets, network access, account permissions, or external side effects without BodyGate.",
            ]);
            let entry = workspace
                .draft_generated_skill(&skill, evidence)
                .map_err(|error| error.to_string())?;
            write_json_output(None, &entry)
        }
        "install" => {
            let name = option_value(&options, "--name");
            let activate =
                parse_bool_option(option_value(&options, "--activate").unwrap_or("false"))?;
            let (markdown, source) = if let Some(path) = option_value(&options, "--file") {
                let path = PathBuf::from(path);
                let markdown = fs::read_to_string(&path).map_err(|error| error.to_string())?;
                (markdown, path.display().to_string())
            } else if let Some(url) = option_value(&options, "--url") {
                (fetch_skill_markdown_url(&root, url)?, url.to_string())
            } else {
                return Err("skills install requires --file PATH or --url URL".to_string());
            };
            let receipt = workspace
                .install_markdown_skill(&markdown, source, name, activate)
                .map_err(|error| error.to_string())?;
            write_json_output(None, &receipt)
        }
        other => Err(format!("unknown skills subcommand `{other}`")),
    }
}

fn parse_bool_option(value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "y" | "on" => Ok(true),
        "false" | "0" | "no" | "n" | "off" => Ok(false),
        other => Err(format!("expected true or false, got `{other}`")),
    }
}

fn fetch_skill_markdown_url(root: &PathBuf, url: &str) -> Result<String, String> {
    let url = normalize_skill_markdown_url(url)?;
    let parsed = reqwest::Url::parse(&url).map_err(|error| error.to_string())?;
    if parsed.scheme() != "https" {
        return Err("skill install URL must use https".to_string());
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "skill install URL has no host".to_string())?
        .to_ascii_lowercase();
    buster_daemon::body_gate_bridge::preflight_network(root, &url, &[host.as_str()])
        .map_err(|error| error.to_string())?;
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("BusterSkillInstaller/0.1")
        .build()
        .map_err(|error| error.to_string())?
        .get(&url)
        .header("Accept", "text/markdown,text/plain,*/*")
        .send();
    let text = match response {
        Ok(response) => {
            let response = response
                .error_for_status()
                .map_err(|error| error.to_string())?;
            if response.content_length().unwrap_or(0) > 256 * 1024 {
                return Err("skill markdown response is too large".to_string());
            }
            response.text().map_err(|error| error.to_string())?
        }
        Err(error) => fetch_text_with_curl(&url, &format!("reqwest failed: {error}"))?,
    };
    if text.chars().count() > 16_000 {
        return Err("skill markdown exceeds 16000 characters".to_string());
    }
    Ok(text)
}

fn fetch_text_with_curl(url: &str, cause: &str) -> Result<String, String> {
    let output = Command::new("curl")
        .args([
            "--location",
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            "30",
            "--max-filesize",
            "262144",
            url,
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("{cause}; curl fallback could not start: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{cause}; curl fallback failed with status {:?}: {}",
            output.status.code(),
            stderr.trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("{cause}; curl fallback returned non-utf8 data: {error}"))
}

fn normalize_skill_markdown_url(url: &str) -> Result<String, String> {
    let parsed = reqwest::Url::parse(url).map_err(|error| error.to_string())?;
    let Some(host) = parsed.host_str() else {
        return Err("skill install URL has no host".to_string());
    };
    if host.eq_ignore_ascii_case("github.com") {
        let segments = parsed
            .path_segments()
            .map(|segments| segments.collect::<Vec<_>>())
            .unwrap_or_default();
        if segments.len() >= 5 && segments[2] == "blob" {
            let owner = segments[0];
            let repo = segments[1];
            let branch = segments[3];
            let path = segments[4..].join("/");
            return Ok(format!(
                "https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{path}"
            ));
        }
    }
    Ok(url.to_string())
}

fn run_tools_command(args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err(
            "tools requires a subcommand: scan | list | query | view | record-use".to_string(),
        );
    };
    let (root, options) = parse_root_and_key_values(&args[1..])?;
    let workspace = ToolWorkspace::new(root);
    match subcommand {
        "scan" => {
            let registry = workspace
                .refresh_registry()
                .map_err(|error| error.to_string())?;
            write_json_output(None, &registry)
        }
        "list" => {
            let registry = workspace
                .read_registry()
                .or_else(|_| workspace.refresh_registry())
                .map_err(|error| error.to_string())?;
            write_json_output(None, &registry.entries)
        }
        "query" => {
            let query = option_value(&options, "--query").unwrap_or("");
            let entries = workspace
                .query_tools(query)
                .map_err(|error| error.to_string())?;
            write_json_output(None, &entries)
        }
        "view" => {
            let name = required_option(&options, "--name")?;
            let view = workspace
                .view_tool(name)
                .map_err(|error| error.to_string())?;
            write_json_output(None, &view)
        }
        "record-use" => {
            let name = required_option(&options, "--name")?;
            let result =
                parse_tool_result(option_value(&options, "--result").unwrap_or("success"))?;
            let summary = option_value(&options, "--summary").unwrap_or("manual tool use");
            let receipt = workspace
                .record_use(name, result, summary)
                .map_err(|error| error.to_string())?;
            write_json_output(None, &receipt)
        }
        other => Err(format!("unknown tools subcommand `{other}`")),
    }
}

fn parse_tool_result(value: &str) -> Result<ToolActionResult, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "success" | "ok" => Ok(ToolActionResult::Success),
        "blocked" => Ok(ToolActionResult::Blocked),
        "failed" | "error" => Ok(ToolActionResult::Failed),
        other => Err(format!(
            "unknown tool result `{other}`; expected success, blocked, or failed"
        )),
    }
}

fn parse_root_arg(args: &[String]) -> Result<PathBuf, String> {
    let mut root = env::current_dir().map_err(|error| error.to_string())?;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                root = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "--root requires a path".to_string())?,
                );
            }
            other => return Err(format!("unknown node option `{other}`")),
        }
        index += 1;
    }
    Ok(root)
}

fn parse_root_and_key_values(args: &[String]) -> Result<(PathBuf, Vec<(String, String)>), String> {
    let mut root = env::current_dir().map_err(|error| error.to_string())?;
    let mut options = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let key = args[index].as_str();
        if !key.starts_with("--") {
            return Err(format!("unknown option `{key}`"));
        }
        index += 1;
        let value = args
            .get(index)
            .ok_or_else(|| format!("{key} requires a value"))?
            .clone();
        if key == "--root" {
            root = PathBuf::from(value);
        } else {
            options.push((key.to_string(), value));
        }
        index += 1;
    }
    Ok((root, options))
}

fn required_option<'a>(options: &'a [(String, String)], key: &str) -> Result<&'a str, String> {
    option_value(options, key).ok_or_else(|| format!("{key} is required"))
}

fn option_value<'a>(options: &'a [(String, String)], key: &str) -> Option<&'a str> {
    options
        .iter()
        .find(|(option, _)| option == key)
        .map(|(_, value)| value.as_str())
}

fn option_values(options: &[(String, String)], key: &str) -> Vec<String> {
    options
        .iter()
        .filter(|(option, _)| option == key)
        .map(|(_, value)| value.clone())
        .collect()
}

fn parse_root_and_optional_file_arg(
    args: &[String],
    file_flag: &str,
) -> Result<(PathBuf, Option<PathBuf>), String> {
    let mut root = env::current_dir().map_err(|error| error.to_string())?;
    let mut file = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                root = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "--root requires a path".to_string())?,
                );
            }
            flag if flag == file_flag => {
                index += 1;
                file = Some(PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| format!("{file_flag} requires a path"))?,
                ));
            }
            other => return Err(format!("unknown node option `{other}`")),
        }
        index += 1;
    }
    Ok((root, file))
}

fn parse_root_and_required_file_arg(
    args: &[String],
    file_flag: &str,
) -> Result<(PathBuf, Option<PathBuf>), String> {
    parse_root_and_optional_file_arg(args, file_flag)
}

fn write_json_output(out: Option<PathBuf>, value: &impl serde::Serialize) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    if let Some(out) = out {
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(&out, text).map_err(|error| error.to_string())?;
        println!("{}", out.display());
    } else {
        println!("{text}");
    }
    Ok(())
}

fn print_tail(path: PathBuf, max_lines: usize) -> Result<(), String> {
    let text = fs::read_to_string(path).unwrap_or_default();
    let mut lines = text.lines().map(ToString::to_string).collect::<Vec<_>>();
    if lines.len() > max_lines {
        lines = lines.split_off(lines.len() - max_lines);
    }
    for line in lines {
        println!("{line}");
    }
    Ok(())
}

fn print_record(record: &buster_daemon::DaemonTickRecord) {
    println!(
        "tick={} body={} action={} score={:.3} level={} reason={}",
        record.tick_index,
        record.body_mode,
        record.selected_action_kind,
        record.value_score,
        record.governance_level,
        record.reason
    );
}

fn print_usage() {
    println!(
        "usage:\n  busterd tick [--root PATH] [--execute-actions] [--max-context-bytes N] [--no-body-reports]\n  busterd run [--root PATH] [--ticks N|--forever] [--interval-ms N] [--execute-actions] [--max-context-bytes N] [--no-body-reports]\n  busterd web [--root PATH] [--port N] [--execute-actions] [--no-open]\n  busterd arena status|competitions [--root PATH]\n  busterd arena onboarding [--root PATH] [--prefer TEXT]\n  busterd arena register [--root PATH] --name NAME --quote QUOTE\n  busterd arena fetch-skill [--root PATH] [--skill-file /skills/arena.md] [--prefer TEXT]\n  busterd arena heartbeat [--root PATH] [--force true|false]\n  busterd arena strategy [--root PATH] [--prefer TEXT]\n  busterd node init|heartbeat|verify|registry [--root PATH]\n  busterd node snapshot [--root PATH] [--out PATH]\n  busterd node witness [--root PATH] --snapshot PATH\n  busterd node import-receipt [--root PATH] --receipt PATH\n  busterd harness tail-contracts|tail-outcomes [--root PATH] [--lines N]\n  busterd papers acquire [--root PATH] --question TEXT [--domain DOMAIN] [--task-id ID] [--limit N]\n  busterd papers brief [--root PATH] --question TEXT [--domain DOMAIN] [--task-id ID] [--max-papers N]\n  busterd papers tail|brief-tail [--root PATH] [--lines N]\n  busterd paperqa doctor|tail [--root PATH] [--lines N]\n  busterd paperqa ask [--root PATH] --question TEXT [--domain DOMAIN] [--task-id ID] [--settings NAME] [--index NAME] [--llm MODEL]\n  busterd web-search search [--root PATH] --query TEXT [--count N] [--region REGION] [--safe-search strict|moderate|off] [--task-id ID]\n  busterd web-search tail [--root PATH] [--lines N]\n  busterd security-research taskflows|audit-local|tail [--root PATH] [--lines N]\n  busterd protocol-security check|tail [--root PATH] [--lines N]\n  busterd skills scan|list [--root PATH]\n  busterd skills install [--root PATH] (--file PATH | --url URL) [--name NAME] [--activate true|false]\n  busterd skills view [--root PATH] --name NAME\n  busterd skills record-use [--root PATH] --name NAME [--context TEXT] [--outcome TEXT]\n  busterd skills draft [--root PATH] --name NAME --trigger TEXT --purpose TEXT --procedure TEXT [--validation TEXT] [--evidence TEXT]\n  busterd tools scan|list [--root PATH]\n  busterd tools query [--root PATH] [--query TEXT]\n  busterd tools view [--root PATH] --name NAME\n  busterd tools record-use [--root PATH] --name NAME [--result success|blocked|failed] [--summary TEXT]"
    );
}
