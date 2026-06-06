use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use buster_body::capabilities::CapabilityLease;
use buster_body::host_api::BodyScope;
use buster_body::resources::{ResourceBudget, ResourceUsage};
use buster_body::runtime::{
    BodyRuntime, RuntimeKind, RuntimeOutcome, RuntimePolicy, RuntimeRequest,
};
use buster_body::{
    AuthorityFileRepairer, AuthorityFileSensor, BodyController, BodyRuntimePaths,
    BodyRuntimeWriter, BodySupervisor, BrokeredHttpToolBackend, DesiredBodyState, EgressBroker,
    LocalEncryptedSecretStore, ManagedUnit, MasterKeyProvider, NetworkEgressSensor,
    NetworkObservation, NetworkPolicy, OutputObservation, ProcessManager, ProcessSpawnRequest,
    ScriptSandbox, ScriptSandboxConfig, SecretBackend, SecretBroker, SecretClass,
    SecretFingerprint, SecretHandle, SecretOutputSensor, SecretRecord, SecretRedactor,
    SecretRegistry, SensorSignal, SensorSuite, SignalKind, StaticSignalSensor, SupervisorConfig,
    UnitKind, UnitStatus, WindowsCredentialManagerBridge,
};

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
        "doctor" => {
            let root = parse_root(&args[1..])?;
            run_doctor(&root)
        }
        "supervise" => {
            let options = parse_supervise_options(&args[1..])?;
            run_supervise(options)
        }
        "recovery-demo" => {
            let source_root = parse_recovery_demo_source(&args[1..])?;
            run_recovery_demo(&source_root)
        }
        "malicious-script-demo" => {
            let root = parse_demo_root(&args[1..])?;
            run_malicious_script_demo(&root)
        }
        "script-demo" => {
            let root = parse_demo_root(&args[1..])?;
            run_script_demo(&root)
        }
        "secret-demo" => run_secret_demo(),
        "secret-store-demo" => {
            let root = parse_demo_root(&args[1..])?;
            run_secret_store_demo(&root)
        }
        "master-key-demo" => run_master_key_demo(),
        "egress-tool-demo" => run_egress_tool_demo(),
        "process-demo" => {
            let root = parse_demo_root(&args[1..])?;
            run_process_demo(&root)
        }
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown command `{other}`")),
    }
}

#[derive(Debug, Clone)]
struct SuperviseOptions {
    root: PathBuf,
    ticks: usize,
    interval_ms: u64,
    write_reports: bool,
}

fn parse_root(args: &[String]) -> Result<PathBuf, String> {
    if args.is_empty() {
        return env::current_dir().map_err(|error| error.to_string());
    }

    if args.len() == 1 {
        return Ok(PathBuf::from(&args[0]));
    }

    Err("doctor accepts at most one project root path".to_string())
}

fn parse_supervise_options(args: &[String]) -> Result<SuperviseOptions, String> {
    let mut root = env::current_dir().map_err(|error| error.to_string())?;
    let mut ticks = 1usize;
    let mut interval_ms = 30_000u64;
    let mut write_reports = false;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--root requires a path".to_string())?;
                root = PathBuf::from(value);
            }
            "--ticks" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--ticks requires a positive integer".to_string())?;
                ticks = value
                    .parse()
                    .map_err(|_| "--ticks must be a positive integer".to_string())?;
            }
            "--interval-ms" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--interval-ms requires a positive integer".to_string())?;
                interval_ms = value
                    .parse()
                    .map_err(|_| "--interval-ms must be a positive integer".to_string())?;
            }
            "--write-reports" => {
                write_reports = true;
            }
            other => return Err(format!("unknown supervise option `{other}`")),
        }
        index += 1;
    }

    Ok(SuperviseOptions {
        root,
        ticks,
        interval_ms,
        write_reports,
    })
}

fn parse_recovery_demo_source(args: &[String]) -> Result<PathBuf, String> {
    if args.is_empty() {
        return env::current_dir().map_err(|error| error.to_string());
    }

    if args.len() == 2 && args[0] == "--source-root" {
        return Ok(PathBuf::from(&args[1]));
    }

    Err("recovery-demo accepts: --source-root PROJECT_ROOT".to_string())
}

fn parse_demo_root(args: &[String]) -> Result<PathBuf, String> {
    if args.is_empty() {
        return env::current_dir().map_err(|error| error.to_string());
    }

    if args.len() == 2 && args[0] == "--root" {
        return Ok(PathBuf::from(&args[1]));
    }

    Err("demo accepts: --root PROJECT_ROOT".to_string())
}

fn run_doctor(root: &Path) -> Result<(), String> {
    let signals = authority_signals(root)?;
    let mut controller = BodyController::new(DesiredBodyState::normal());
    let report = controller.reconcile(signals);

    print_report("doctor", &report);
    Ok(())
}

fn run_recovery_demo(source_root: &Path) -> Result<(), String> {
    for file_name in ["self.md", "GOVERNANCE.md", "BODY.md"] {
        let path = source_root.join(file_name);
        if !path.exists() {
            return Err(format!(
                "source root is missing required authority file `{}`",
                path.display()
            ));
        }
    }

    let demo_root = unique_demo_root();
    fs::create_dir_all(&demo_root).map_err(|error| error.to_string())?;
    fs::copy(source_root.join("self.md"), demo_root.join("self.md"))
        .map_err(|error| error.to_string())?;
    fs::copy(
        source_root.join("GOVERNANCE.md"),
        demo_root.join("GOVERNANCE.md"),
    )
    .map_err(|error| error.to_string())?;

    println!("demo root: {}", demo_root.display());
    println!("demo setup: BODY.md intentionally missing");

    let mut controller = BodyController::new(DesiredBodyState::normal());
    let writer = BodyRuntimeWriter::new(BodyRuntimePaths::from_project_root(&demo_root));

    let tick0_signals = authority_signals(&demo_root)?;
    let tick0 = controller.reconcile(tick0_signals);
    let tick0_paths = writer
        .write_report(&tick0)
        .map_err(|error| error.to_string())?;
    print_report("demo tick 0 detect", &tick0);
    print_written(&tick0_paths);

    let repairer = AuthorityFileRepairer::new(&demo_root, source_root);
    let repairs = repairer
        .repair_missing_files(&tick0.signals)
        .map_err(|error| error.to_string())?;
    println!("repairs:");
    if repairs.is_empty() {
        println!("- none");
    } else {
        for repair in repairs {
            println!(
                "- {} <- {} ({})",
                repair.target.display(),
                repair.source.display(),
                repair.summary
            );
        }
    }

    let tick1 = controller.reconcile(authority_signals(&demo_root)?);
    let tick1_paths = writer
        .write_report(&tick1)
        .map_err(|error| error.to_string())?;
    print_report("demo tick 1 recovery", &tick1);
    print_written(&tick1_paths);

    let tick2 = controller.reconcile(authority_signals(&demo_root)?);
    let tick2_paths = writer
        .write_report(&tick2)
        .map_err(|error| error.to_string())?;
    print_report("demo tick 2 restored", &tick2);
    print_written(&tick2_paths);

    fs::remove_dir_all(&demo_root).map_err(|error| error.to_string())?;
    println!("demo cleanup: removed {}", demo_root.display());

    Ok(())
}

fn run_malicious_script_demo(root: &Path) -> Result<(), String> {
    println!("malicious script demo root: {}", root.display());
    println!(
        "demo setup: simulated Buster-owned script attempts suspicious egress and secret output"
    );

    let writer = BodyRuntimeWriter::new(BodyRuntimePaths::from_project_root(root));
    let mut controller = BodyController::new(DesiredBodyState::normal());
    let mut unit = ManagedUnit::new("demo-script", UnitKind::Script, "buster")
        .with_status(UnitStatus::Running);
    unit.notes
        .push("suspicious network anomaly and secret exfiltration attempt".to_string());
    controller.register_unit(unit);

    let sensors = SensorSuite::new()
        .with_sensor(NetworkEgressSensor::new(
            vec![NetworkObservation {
                unit_id: "demo-script".to_string(),
                host: "evil.example".to_string(),
            }],
            vec!["openrouter.ai".to_string()],
        ))
        .with_sensor(SecretOutputSensor::new(vec![OutputObservation {
            unit_id: "demo-script".to_string(),
            text: "exfiltrating token sk-demo-redacted".to_string(),
        }]));

    let signals = sensors.scan();
    let report = controller.reconcile(signals);
    let written = writer
        .write_report(&report)
        .map_err(|error| error.to_string())?;

    print_report("malicious script detect", &report);
    print_written(&written);

    Ok(())
}

fn run_script_demo(root: &Path) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|error| error.to_string())?;
    let script_path = root.join("demo-malicious-script.sh");
    fs::write(
        &script_path,
        "#!/usr/bin/env bash\n\
         echo 'NETWORK evil.example'\n\
         echo 'tool output contains sk-demo-redacted'\n",
    )
    .map_err(|error| error.to_string())?;

    println!("script demo root: {}", root.display());
    println!("script demo file: {}", script_path.display());

    let sandbox = ScriptSandbox::new(ScriptSandboxConfig {
        command: "bash".to_string(),
        args: vec![script_path.to_string_lossy().into_owned()],
        cwd: Some(root.to_path_buf()),
        timeout: Duration::from_secs(3),
        allowed_hosts: vec!["openrouter.ai".to_string()],
    });
    let run = sandbox.run("demo-script").map_err(|error| error.reason)?;

    println!("captured stdout:");
    print!("{}", run.stdout);
    if !run.stderr.is_empty() {
        println!("captured stderr:");
        print!("{}", run.stderr);
    }

    let writer = BodyRuntimeWriter::new(BodyRuntimePaths::from_project_root(root));
    let mut controller = BodyController::new(DesiredBodyState::normal());
    controller.register_unit(run.unit);
    let report = controller.reconcile(run.signals);
    let written = writer
        .write_report(&report)
        .map_err(|error| error.to_string())?;

    print_report("script demo detect", &report);
    print_written(&written);

    Ok(())
}

fn run_supervise(options: SuperviseOptions) -> Result<(), String> {
    let initial_signals = authority_signals(&options.root)?;
    let sensors = if initial_signals.is_empty() {
        SensorSuite::new().with_sensor(authority_sensor(&options.root)?)
    } else {
        SensorSuite::new().with_sensor(StaticSignalSensor::new(initial_signals.clone()))
    };
    let controller = BodyController::new(DesiredBodyState::normal());

    let writer = if options.write_reports {
        Some(BodyRuntimeWriter::new(BodyRuntimePaths::from_project_root(
            &options.root,
        )))
    } else {
        None
    };
    let config = SupervisorConfig {
        tick_interval: Duration::from_millis(options.interval_ms),
        max_ticks: Some(options.ticks),
        write_reports: options.write_reports,
    };
    let mut supervisor = BodySupervisor::new(controller, sensors, writer, config);
    let ticks = supervisor.run().map_err(|error| error.to_string())?;

    for tick in ticks {
        print_report(&format!("tick {}", tick.index), &tick.report);
        if !tick.written_paths.is_empty() {
            println!("written:");
            for path in tick.written_paths {
                println!("- {}", path.display());
            }
        }
    }

    Ok(())
}

fn run_secret_demo() -> Result<(), String> {
    let mut registry = SecretRegistry::new();
    let openrouter = SecretHandle("openrouter_api_key".to_string());
    registry.register(
        SecretRecord::new(
            openrouter.clone(),
            SecretClass::LlmApiKey,
            vec!["llm.openrouter".to_string()],
            "OpenRouter API key handle; plaintext lives in an external secret backend",
        )
        .with_fingerprint(SecretFingerprint::from_secret(
            "sk-demo-openrouter-secret",
            b"demo-fingerprint-key",
        )),
    );
    registry.register(SecretRecord::new(
        SecretHandle("buster_identity_key".to_string()),
        SecretClass::IdentityKey,
        vec!["identity.prove".to_string()],
        "Buster identity key; not leasable as an ordinary secret",
    ));

    let lease = registry
        .issue_lease(
            &openrouter,
            "llm.openrouter",
            Some(600),
            "demo inference",
            buster_body::BodyMode::Normal,
        )
        .map_err(|error| format!("failed to issue lease: {error:?}"))?;

    println!("registered secret handle: {}", openrouter.0);
    println!("issued lease: {}", lease.lease_id);
    println!("lease capability: {}", lease.capability);
    println!("lease reason: {}", lease.reason);
    println!("plaintext secret exposed: no");

    let decision = registry.consume_lease(
        &lease.lease_id,
        "llm.openrouter",
        buster_body::BodyMode::Normal,
    );
    println!("consume decision: {decision:?}");

    let blocked = registry.issue_lease(
        &SecretHandle("buster_identity_key".to_string()),
        "identity.prove",
        Some(60),
        "demo identity proof",
        buster_body::BodyMode::Normal,
    );
    println!("identity key lease attempt: {blocked:?}");

    let redactor = SecretRedactor::new(registry.fingerprints(), b"demo-fingerprint-key".to_vec());
    let leaked_line = "adapter log accidentally included sk-demo-openrouter-secret";
    println!(
        "registered fingerprint leak detected: {}",
        redactor.detects_leak(leaked_line)
    );
    println!("redacted adapter log: {}", redactor.redact(leaked_line));

    Ok(())
}

fn run_secret_store_demo(root: &Path) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|error| error.to_string())?;
    let store_path = root.join("secrets").join("buster-secrets.json");
    let master_key = buster_body::default_master_key().map_err(|error| error.to_string())?;
    let handle = SecretHandle("openrouter_api_key".to_string());
    let mut store = LocalEncryptedSecretStore::new(&store_path, &master_key);
    store.put_secret(handle.clone(), "sk-demo-local-secret".to_string());

    let mut registry = SecretRegistry::new();
    registry.register(SecretRecord::new(
        handle.clone(),
        SecretClass::LlmApiKey,
        vec!["llm.openrouter".to_string()],
        "stored in Buster local encrypted secret store",
    ));
    let lease = registry
        .issue_lease(
            &handle,
            "llm.openrouter",
            Some(60),
            "local encrypted store demo",
            buster_body::BodyMode::Normal,
        )
        .map_err(|error| format!("failed to issue lease: {error:?}"))?;
    let mut broker = SecretBroker::new(registry, store);

    let header_shape = broker
        .use_secret_once(
            &lease.lease_id,
            "llm.openrouter",
            buster_body::BodyMode::Normal,
            |secret| format!("Bearer len={}", secret.len()),
        )
        .map_err(|error| format!("failed to use secret: {error:?}"))?;

    let raw = fs::read_to_string(&store_path).map_err(|error| error.to_string())?;
    println!("local secret store: {}", store_path.display());
    println!(
        "plaintext appears in store file: {}",
        raw.contains("sk-demo-local-secret")
    );
    println!("broker received secret shape: {header_shape}");

    Ok(())
}

fn run_master_key_demo() -> Result<(), String> {
    let provider = WindowsCredentialManagerBridge::default_buster();
    let key = provider
        .get_or_create_master_key()
        .map_err(|error| error.to_string())?;

    println!("windows credential target: Buster.Body.MasterKey");
    println!("master key length: {} bytes", key.len());
    println!("master key plaintext printed: no");

    Ok(())
}

fn run_process_demo(root: &Path) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|error| error.to_string())?;
    println!("process demo root: {}", root.display());
    println!(
        "demo setup: spawn a Buster-owned sleeper, detect a critical violation, then contain it"
    );

    let mut manager = ProcessManager::new();
    let mut unit = manager
        .spawn(demo_sleep_request(root))
        .map_err(|error| error.to_string())?;
    unit.notes
        .push("suspicious policy violation from managed process".to_string());
    let status = manager
        .status("demo-managed-process")
        .map_err(|error| error.to_string())?;
    println!(
        "spawned managed unit: {} pid={} status={:?}",
        status.unit_id, status.pid, status.status
    );

    let writer = BodyRuntimeWriter::new(BodyRuntimePaths::from_project_root(root));
    let mut controller = BodyController::new(DesiredBodyState::normal());
    controller.register_unit(unit);
    let report = controller.reconcile(vec![SensorSignal::critical(
        SignalKind::PolicyViolation,
        "demo-managed-process",
        "managed process attempted a forbidden body-layer action",
    )]);

    let mut terminated = Vec::new();
    for action in &report.actions {
        if let buster_body::responder::BodyAction::FreezeOwnedUnit { unit_id, .. } = action {
            let snapshot = manager
                .terminate_owned(unit_id)
                .map_err(|error| error.to_string())?;
            terminated.push(snapshot);
        }
    }

    let written = writer
        .write_report(&report)
        .map_err(|error| error.to_string())?;

    print_report("process demo contain", &report);
    if terminated.is_empty() {
        println!("terminated: none");
    } else {
        println!("terminated:");
        for snapshot in terminated {
            println!(
                "- {} pid={} status={:?}",
                snapshot.unit_id, snapshot.pid, snapshot.status
            );
        }
    }
    print_written(&written);

    Ok(())
}

fn run_egress_tool_demo() -> Result<(), String> {
    let scope = BodyScope::new("buster", "main", "egress-tool-demo");
    let broker = EgressBroker::new(NetworkPolicy {
        allowed_hosts: vec!["openrouter.ai".to_string()],
        deny_private_networks: true,
        max_response_bytes: Some(1024),
    });
    let backend = BrokeredHttpToolBackend::new(
        broker,
        "demo-http-tool",
        serde_json::json!({ "demo": "blocked-before-network" }),
    );
    let runtime = BodyRuntime::new(backend);
    let request = RuntimeRequest::new(
        scope.clone(),
        RuntimeKind::Tool,
        "http.post_json",
        "https://evil.example/api",
    )
    .with_network()
    .with_estimated_usage(ResourceUsage {
        network_bytes: 256,
        ..ResourceUsage::default()
    });
    let policy = RuntimePolicy::deny_by_default(ResourceBudget {
        max_network_bytes: Some(1024),
        ..ResourceBudget::default()
    })
    .with_network()
    .with_lease(CapabilityLease {
        scope,
        capability_name: "http.post_json".to_string(),
        expires_at: None,
        reason: "egress broker demo".to_string(),
    });

    match runtime.invoke(request, &policy) {
        RuntimeOutcome::Failed(failure) => {
            println!("egress tool blocked before network: {}", failure.reason);
            Ok(())
        }
        RuntimeOutcome::Blocked(block) => {
            println!("runtime preflight blocked request: {:?}", block.reason);
            Ok(())
        }
        RuntimeOutcome::Completed(completion) => Err(format!(
            "unexpected completion: {}",
            completion.output_summary
        )),
    }
}

fn authority_signals(root: &Path) -> Result<Vec<SensorSignal>, String> {
    let mut signals = Vec::new();

    for file_name in ["self.md", "GOVERNANCE.md", "BODY.md"] {
        let path = root.join(file_name);
        if !path.exists() {
            signals.push(SensorSignal::critical(
                SignalKind::AuthorityFileMissing,
                path.display().to_string(),
                "required authority file is missing",
            ));
        }
    }

    if signals.is_empty() {
        let sensor = authority_sensor(root)?;
        signals.extend(sensor.scan());
    }

    Ok(signals)
}

fn authority_sensor(root: &Path) -> Result<AuthorityFileSensor, String> {
    let paths = ["self.md", "GOVERNANCE.md", "BODY.md"]
        .into_iter()
        .map(|file_name| root.join(file_name));

    AuthorityFileSensor::from_paths(paths).map_err(|error| error.to_string())
}

fn print_report(label: &str, report: &buster_body::ReconcileReport) {
    println!(
        "{label}: desired={} previous={} next={} signals={} actions={}",
        report.desired_mode.as_str(),
        report.previous_mode.as_str(),
        report.next_mode.as_str(),
        report.signals.len(),
        report.actions.len()
    );

    if !report.signals.is_empty() {
        println!("signals:");
        for signal in &report.signals {
            println!(
                "- {:?} {:?} from {}: {}",
                signal.severity, signal.kind, signal.source, signal.summary
            );
        }
    }

    if !report.actions.is_empty() {
        println!("actions:");
        for action in &report.actions {
            println!("- {action:?}");
        }
    }
}

fn print_written(paths: &[PathBuf]) {
    if paths.is_empty() {
        println!("written: none");
        return;
    }

    println!("written:");
    for path in paths {
        println!("- {}", path.display());
    }
}

fn print_usage() {
    println!(
        "buster-body\n\n\
         Commands:\n\
         buster-body doctor [PROJECT_ROOT]\n\
         buster-body supervise --root PROJECT_ROOT [--ticks N] [--interval-ms N] [--write-reports]\n\
         buster-body recovery-demo --source-root PROJECT_ROOT\n\
         buster-body malicious-script-demo --root PROJECT_ROOT\n\
         buster-body script-demo --root PROJECT_ROOT\n\
         buster-body secret-demo\n\
         buster-body secret-store-demo --root PROJECT_ROOT\n\
         buster-body master-key-demo\n\
         buster-body egress-tool-demo\n\
         buster-body process-demo --root PROJECT_ROOT\n"
    );
}

fn unique_demo_root() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    env::temp_dir().join(format!("buster-body-recovery-demo-{stamp}"))
}

fn demo_sleep_request(root: &Path) -> ProcessSpawnRequest {
    #[cfg(windows)]
    {
        ProcessSpawnRequest::new("demo-managed-process", UnitKind::Script, "buster", "cmd")
            .with_args(["/C", "ping", "-n", "30", "127.0.0.1", ">NUL"])
            .with_cwd(root)
            .inherit_env()
    }

    #[cfg(not(windows))]
    {
        ProcessSpawnRequest::new(
            "demo-managed-process",
            UnitKind::Script,
            "buster",
            "/bin/sh",
        )
        .with_args(["-c", "sleep 30"])
        .with_cwd(root)
    }
}
