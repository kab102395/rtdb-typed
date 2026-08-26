//! Smoke test for the running Firebase RTDB emulator.
//!
//! This remains ignored because it needs the Firebase CLI to start the
//! emulator. Run it through `scripts/test-emulator.sh`, never a real project.

use rtdb_typed::{FirebaseCollection, TypedClient};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Health {
    ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Record {
    worker: u32,
    sequence: u32,
    active: bool,
}

#[tokio::test]
#[ignore]
async fn emulator_is_local_and_responds() {
    let url = "http://127.0.0.1:9000/healthcheck.json?ns=demo-rtdb-typed";
    let response = reqwest::Client::new()
        .put(url)
        .json(&serde_json::json!({ "ok": true }))
        .send()
        .await
        .expect("database emulator must be running");
    assert!(
        response.status().is_success(),
        "emulator response: {}",
        response.status()
    );

    let client = TypedClient::from_parts("http://127.0.0.1:9000", "local-emulator");
    let health: Health = client
        .put("typed-health", &Health { ok: true })
        .await
        .expect("typed PUT should reach the local emulator");
    assert_eq!(health, Health { ok: true });
    assert_eq!(client.get::<Health>("typed-health").await.unwrap(), health);
    client.delete("typed-health").await.unwrap();
}

#[tokio::test]
#[ignore]
async fn emulator_stress_standard_profile() {
    run_stress_profile(32, 50).await.unwrap();
}

#[tokio::test]
#[ignore]
async fn emulator_stress_high_profile_manual() {
    run_stress_profile(64, 100).await.unwrap();
}

async fn run_stress_profile(
    workers: u32,
    operations_per_worker: u32,
) -> Result<(), rtdb_typed::TypedError> {
    let started = Instant::now();
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let records_root = "stress_records".to_string();
    let posts_root = format!("stress_posts/{run_id}");
    let client = Arc::new(TypedClient::from_parts("http://127.0.0.1:9000", ""));

    let mut tasks = Vec::new();
    for worker in 0..workers {
        let client = Arc::clone(&client);
        let records_root = records_root.clone();
        let posts_root = posts_root.clone();
        tasks.push(tokio::spawn(async move {
            for sequence in 0..operations_per_worker {
                let path = format!("{records_root}/{run_id}-{worker}-{sequence}");
                let record = Record {
                    worker,
                    sequence,
                    active: true,
                };
                let written: Record = client.put(&path, &record).await?;
                assert_eq!(written, record);
                assert_eq!(client.get::<Record>(&path).await?, record);

                let updated = serde_json::json!({ "active": false });
                let _: serde_json::Value = client.patch(&path, &updated).await?;
                let patched = client.get::<Record>(&path).await?;
                assert!(!patched.active);

                let key = client.post(&posts_root, &record).await?;
                assert!(!key.key.is_empty());

                // Firebase indexes child keys automatically, so key ordering
                // provides a deterministic indexed query for every run.
                let by_key: FirebaseCollection<Record> = client
                    .query(&records_root)
                    .order_by_key()
                    .start_at(rtdb_typed::rtdb_rs::FilterValue::string(format!(
                        "{run_id}-"
                    )))
                    .send_collection()
                    .await?;
                assert!(by_key
                    .values()
                    .all(|item| item.sequence < operations_per_worker));

                let bounded: FirebaseCollection<Record> = client
                    .query(&records_root)
                    .order_by_key()
                    .start_at(rtdb_typed::rtdb_rs::FilterValue::string(format!(
                        "{run_id}-"
                    )))
                    .end_at(rtdb_typed::rtdb_rs::FilterValue::string(format!(
                        "{run_id}-~"
                    )))
                    .limit_to_first(5)
                    .send_collection()
                    .await?;
                assert!(bounded.len() <= 5);
            }
            Ok::<(), rtdb_typed::TypedError>(())
        }));
    }

    for task in tasks {
        task.await
            .expect("stress worker did not panic")
            .expect("stress request failed");
    }

    let records: FirebaseCollection<Record> = client.get_collection(&records_root).await.unwrap();
    assert_eq!(records.len(), (workers * operations_per_worker) as usize);
    assert!(records.values().all(|record| !record.active));

    client.delete(&records_root).await.unwrap();
    client.delete(&posts_root).await.unwrap();
    println!(
        "stress profile: workers={workers} sequences_per_worker={operations_per_worker} sequences={} elapsed_ms={}",
        workers * operations_per_worker,
        started.elapsed().as_millis()
    );
    Ok(())
}
