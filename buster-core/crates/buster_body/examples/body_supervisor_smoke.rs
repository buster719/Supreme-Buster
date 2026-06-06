use std::time::Duration;

use buster_body::{
    BodyController, BodyMode, BodySupervisor, DesiredBodyState, NetworkEgressSensor,
    NetworkObservation, SensorSuite, SupervisorConfig,
};

fn main() {
    let controller = BodyController::new(DesiredBodyState::normal());
    let sensors = SensorSuite::new().with_sensor(NetworkEgressSensor::new(
        vec![NetworkObservation {
            unit_id: "example-tool".to_string(),
            host: "unexpected.example".to_string(),
        }],
        vec!["openrouter.ai".to_string()],
    ));
    let config = SupervisorConfig {
        tick_interval: Duration::from_millis(0),
        max_ticks: Some(1),
        write_reports: false,
    };
    let mut supervisor = BodySupervisor::new(controller, sensors, None, config);
    let ticks = supervisor.run().expect("supervisor should run");

    for tick in ticks {
        println!(
            "tick={} mode={} signals={} actions={}",
            tick.index,
            tick.report.next_mode.as_str(),
            tick.report.signals.len(),
            tick.report.actions.len()
        );
        assert_eq!(tick.report.next_mode, BodyMode::Restricted);
    }
}
