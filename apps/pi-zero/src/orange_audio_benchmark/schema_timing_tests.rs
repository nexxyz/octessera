use super::*;

type SemanticCase = (&'static str, fn(&mut serde_json::Value));

#[test]
fn schema8_rejects_impossible_worker_timing_relationships() {
    let cases: [SemanticCase; 37] = [
        ("missing deadline", |value| {
            value["worker_timing"]["coordinator"]["deadline_ns"] = serde_json::Value::Null;
        }),
        ("missing dispatch-to-deadline start", |value| {
            value["worker_timing"]["coordinator"]["dispatch_to_deadline_start_ns"] =
                serde_json::Value::Null;
        }),
        ("missing in-flight mask", |value| {
            value["worker_timing"]["coordinator"]["in_flight_mask"] = serde_json::Value::Null;
        }),
        ("missing completed mask", |value| {
            value["worker_timing"]["coordinator"]["completed_mask"] = serde_json::Value::Null;
        }),
        ("missing engine total", |value| {
            value["worker_timing"]["coordinator"]["engine_block_total_ns"] =
                serde_json::Value::Null;
        }),
        ("missing callback total", |value| {
            value["worker_timing"]["coordinator"]["callback_total_ns"] = serde_json::Value::Null;
        }),
        ("unknown mask bit", |value| {
            value["worker_timing"]["coordinator"]["in_flight_mask"] = 4.into();
        }),
        ("overlapping masks", |value| {
            value["worker_timing"]["coordinator"]["in_flight_mask"] = 1.into();
        }),
        ("mask union gap", |value| {
            value["worker_timing"]["coordinator"]["completed_mask"] = 1.into();
            value["worker_timing"]["coordinator"]["in_flight_mask"] = 0.into();
        }),
        ("engine total exceeds callback", |value| {
            value["worker_timing"]["coordinator"]["engine_block_total_ns"] = 51.into();
        }),
        ("unexecuted coordinator has measurements", |value| {
            value["worker_timing"]["coordinator"]["sequence"] = serde_json::Value::Null;
        }),
        ("unexecuted coordinator has finished worker", |value| {
            let coordinator = &mut value["worker_timing"]["coordinator"];
            for name in [
                "sequence",
                "deadline_ns",
                "dispatch_to_deadline_start_ns",
                "dispatch_to_deadline_elapsed_ns",
                "in_flight_mask",
                "completed_mask",
                "first_parity",
                "dispatch_to_first_ns",
                "dispatch_to_both_ns",
                "reduction_ns",
                "coordinator_remainder_ns",
                "engine_block_total_ns",
                "callback_total_ns",
            ] {
                coordinator[name] = serde_json::Value::Null;
            }
        }),
        ("zero completed has first evidence", |value| {
            value["worker_timing"]["coordinator"]["completed_mask"] = 0.into();
            value["worker_timing"]["coordinator"]["in_flight_mask"] = 3.into();
        }),
        ("completed has no first evidence", |value| {
            value["worker_timing"]["coordinator"]["first_parity"] = serde_json::Value::Null;
        }),
        ("first parity is not completed", |value| {
            value["worker_timing"]["coordinator"]["first_parity"] = 1.into();
        }),
        ("both completion is missing", |value| {
            value["worker_timing"]["coordinator"]["dispatch_to_both_ns"] = serde_json::Value::Null;
        }),
        ("both completion is premature", |value| {
            value["worker_timing"]["coordinator"]["completed_mask"] = 1.into();
        }),
        ("first follows both", |value| {
            value["worker_timing"]["coordinator"]["dispatch_to_first_ns"] = 30.into();
        }),
        ("first precedes worker finish", |value| {
            value["worker_timing"]["coordinator"]["dispatch_to_first_ns"] = 19.into();
        }),
        ("both precedes worker finish", |value| {
            value["worker_timing"]["coordinator"]["dispatch_to_both_ns"] = 24.into();
        }),
        ("completion is after deadline", |value| {
            value["worker_timing"]["workers"][0]["dispatch_to_finish_ns"] = 111.into();
            value["worker_timing"]["coordinator"]["dispatch_to_first_ns"] = 111.into();
        }),
        ("healthy masks are incomplete", |value| {
            value["worker_timing"]["coordinator"]["in_flight_mask"] = 2.into();
            value["worker_timing"]["coordinator"]["completed_mask"] = 1.into();
        }),
        ("healthy reduction is missing", |value| {
            value["worker_timing"]["coordinator"]["reduction_ns"] = serde_json::Value::Null;
        }),
        ("healthy remainder is missing", |value| {
            value["worker_timing"]["coordinator"]["coordinator_remainder_ns"] =
                serde_json::Value::Null;
        }),
        ("healthy deadline elapsed is present", |value| {
            value["worker_timing"]["coordinator"]["dispatch_to_deadline_elapsed_ns"] = 110.into();
        }),
        ("failed deadline elapsed precedes deadline", |value| {
            value["worker_timing"]["coordinator"]["failed"] = true.into();
            value["worker_timing"]["coordinator"]["dispatch_to_deadline_elapsed_ns"] = 109.into();
        }),
        (
            "failed deadline elapsed precedes dispatch boundary",
            |value| {
                value["worker_timing"]["coordinator"]["failed"] = true.into();
                value["worker_timing"]["coordinator"]["dispatch_to_deadline_start_ns"] = 50.into();
                value["worker_timing"]["coordinator"]["dispatch_to_deadline_elapsed_ns"] =
                    149.into();
            },
        ),
        ("failed incomplete timing has reduction", |value| {
            value["worker_timing"]["coordinator"]["failed"] = true.into();
            value["worker_timing"]["coordinator"]["in_flight_mask"] = 2.into();
            value["worker_timing"]["coordinator"]["completed_mask"] = 1.into();
            value["worker_timing"]["coordinator"]["dispatch_to_both_ns"] = serde_json::Value::Null;
        }),
        ("remainder has no reduction", |value| {
            value["worker_timing"]["coordinator"]["failed"] = true.into();
            value["worker_timing"]["coordinator"]["reduction_ns"] = serde_json::Value::Null;
        }),
        ("finished worker sequence is missing", |value| {
            value["worker_timing"]["workers"][0]["sequence"] = serde_json::Value::Null;
        }),
        ("finished worker sequence disagrees", |value| {
            value["worker_timing"]["workers"][1]["sequence"] = 8.into();
        }),
        ("worker dispatch precedes render", |value| {
            value["worker_timing"]["workers"][0]["dispatch_to_finish_ns"] = 9.into();
        }),
        ("unfinished worker has evidence", |value| {
            value["worker_timing"]["workers"][0]["finished"] = false.into();
        }),
        ("worker CPU pair is partial", |value| {
            value["worker_timing"]["workers"][0]["cpu_start"] = serde_json::Value::Null;
        }),
        ("worker CPU availability disagrees", |value| {
            value["worker_timing"]["workers"][1]["cpu_start"] = serde_json::Value::Null;
            value["worker_timing"]["workers"][1]["cpu_end"] = serde_json::Value::Null;
        }),
        ("CPU endpoint-change summary disagrees", |value| {
            value["worker_timing"]["cpu_endpoint_changed"] = false.into();
        }),
        ("late summary disagrees", |value| {
            value["worker_timing"]["late_after_deadline_ns"] = 1.into();
        }),
    ];
    for (name, mutate) in cases {
        let mut value = serde_json::to_value(benchmark_result(
            WorkerTimingMode::Enabled,
            Some(worker_timing()),
        ))
        .unwrap();
        mutate(&mut value);
        assert!(
            serde_json::from_value::<BenchmarkResult>(value).is_err(),
            "case should be rejected: {name}"
        );
    }
}
