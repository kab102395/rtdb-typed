//! Smoke test for the running Firebase RTDB emulator.
//!
//! This remains ignored because it needs the Firebase CLI to start the
//! emulator. Run it through `scripts/test-emulator.sh`, never a real project.

use rtdb_typed::TypedClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

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
async fn emulator_handles_concurrent_typed_crud_load() {
    const WORKERS: u32 = 16;
    const OPERATIONS_PER_WORKER: u32 = 25;
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let root = format!("stress/{run_id}");
    let client = Arc::new(TypedClient::from_parts("http://127.0.0.1:9000", ""));

    let mut tasks = Vec::new();
    for worker in 0..WORKERS {
        let client = Arc::clone(&client);
        let root = root.clone();
        tasks.push(tokio::spawn(async move {
            for sequence in 0..OPERATIONS_PER_WORKER {
                let path = format!("{root}/records/{worker}-{sequence}");
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

                let key = client.post(&format!("{root}/posted"), &record).await?;
                assert!(!key.is_empty());
            }
            Ok::<(), rtdb_typed::TypedError>(())
        }));
    }

    for task in tasks {
        task.await
            .expect("stress worker did not panic")
            .expect("stress request failed");
    }

    let records: HashMap<String, Record> = client
        .get_collection(&format!("{root}/records"))
        .await
        .unwrap();
    assert_eq!(records.len(), (WORKERS * OPERATIONS_PER_WORKER) as usize);
    assert!(records.values().all(|record| !record.active));

    client.delete(&root).await.unwrap();
}
