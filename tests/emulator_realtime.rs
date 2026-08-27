//! Ignored end-to-end realtime tests for the local Firebase RTDB emulator.
//!
//! These tests measure semantic delivery and conversion behavior only. They
//! are not production throughput or latency benchmarks.

use futures_util::StreamExt;
use rtdb_typed::{TypedClient, TypedEvent};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::Barrier;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Record {
    stream: u32,
    revision: u32,
    active: bool,
}

const EVENT_TIMEOUT: Duration = Duration::from_secs(15);
const EMULATOR_NAMESPACE: &str = "demo-rtdb-typed";

fn emulator_client() -> TypedClient {
    TypedClient::new(
        rtdb_typed::rtdb_rs::RtdbClient::new("http://127.0.0.1:9000", "")
            .with_namespace(EMULATOR_NAMESPACE),
    )
}

#[tokio::test]
#[ignore]
async fn emulator_realtime_standard_profile() {
    run_stream_profile(24, 40).await.unwrap();
}

#[tokio::test]
#[ignore]
async fn emulator_realtime_multipath_profile_manual() {
    run_stream_profile(64, 100).await.unwrap();
}

async fn run_stream_profile(stream_count: u32, mutations_per_stream: u32) -> Result<(), String> {
    let started = Instant::now();
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let root = format!("realtime/{run_id}");
    let client = Arc::new(emulator_client());
    let barrier = Arc::new(Barrier::new(stream_count as usize));
    let mut tasks = Vec::new();

    for stream_id in 0..stream_count {
        let client = Arc::clone(&client);
        let barrier = Arc::clone(&barrier);
        let path = format!("{root}/streams/{stream_id}");
        tasks.push(tokio::spawn(async move {
            let stream = client
                .stream::<Record>(&path)
                .await
                .map_err(|error| error.to_string())?;
            tokio::pin!(stream);
            barrier.wait().await;

            let mut events = 0u32;
            let initial = next_event(&mut stream).await?;
            assert!(matches!(initial, TypedEvent::Put { data: None, .. }));
            events += 1;

            let initial_record = Record {
                stream: stream_id,
                revision: 0,
                active: true,
            };
            let written: Record = client
                .put(&path, &initial_record)
                .await
                .map_err(|error| error.to_string())?;
            assert_eq!(written, initial_record);
            assert!(matches!(
                next_event(&mut stream).await?,
                TypedEvent::Put { data: Some(_), .. }
            ));
            events += 1;

            for revision in 1..=mutations_per_stream {
                if revision % 2 == 0 {
                    let value = Record {
                        stream: stream_id,
                        revision,
                        active: true,
                    };
                    let written: Record = client
                        .put(&path, &value)
                        .await
                        .map_err(|error| error.to_string())?;
                    assert_eq!(written, value);
                    match next_event(&mut stream).await? {
                        TypedEvent::Put {
                            data: Some(data), ..
                        } => assert_eq!(data, value),
                        other => panic!("expected typed PUT, got {other:?}"),
                    }
                } else {
                    let patch = json!({"revision": revision, "active": false});
                    let _: serde_json::Value = client
                        .patch(&path, &patch)
                        .await
                        .map_err(|error| error.to_string())?;
                    match next_event(&mut stream).await? {
                        TypedEvent::Patch { data: patch, .. } => {
                            assert_eq!(
                                patch
                                    .deserialize_field::<u32>("revision")
                                    .map_err(|error| error.to_string())?,
                                Some(revision)
                            );
                            assert_eq!(
                                patch
                                    .deserialize_field::<bool>("active")
                                    .map_err(|error| error.to_string())?,
                                Some(false)
                            );
                        }
                        other => panic!("expected typed PATCH, got {other:?}"),
                    }
                }
                events += 1;
            }

            client
                .delete(&path)
                .await
                .map_err(|error| error.to_string())?;
            assert!(matches!(
                next_event(&mut stream).await?,
                TypedEvent::Put { data: None, .. }
            ));
            events += 1;
            Ok::<u32, String>(events)
        }));
    }

    let mut event_count = 0u32;
    for task in tasks {
        event_count += task.await.map_err(|error| error.to_string())??;
    }
    client
        .delete(&root)
        .await
        .map_err(|error| error.to_string())?;
    let mutation_count = stream_count * mutations_per_stream;
    println!(
        "realtime stress: streams={stream_count} mutations={mutation_count} events={event_count} parse_conversion_failures=0 missed_or_extra_events=0 elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(())
}

async fn next_event<S, T>(stream: &mut S) -> Result<TypedEvent<T>, String>
where
    S: futures_util::Stream<Item = Result<TypedEvent<T>, rtdb_typed::TypedError>> + Unpin,
{
    tokio::time::timeout(EVENT_TIMEOUT, stream.next())
        .await
        .map_err(|_| "timed out waiting for realtime event".to_string())?
        .ok_or_else(|| "realtime stream closed before expected event".to_string())?
        .map_err(|error| error.to_string())
}

#[tokio::test]
#[ignore]
async fn emulator_realtime_fanout_profile() {
    run_fanout_profile(32, 100).await.unwrap();
}

async fn run_fanout_profile(subscriber_count: u32, mutation_count: u32) -> Result<(), String> {
    let started = Instant::now();
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let path = format!("realtime-fanout/{run_id}");
    let client = Arc::new(emulator_client());
    let barrier = Arc::new(Barrier::new(subscriber_count as usize + 1));
    let mut tasks = Vec::new();

    for _ in 0..subscriber_count {
        let client = Arc::clone(&client);
        let barrier = Arc::clone(&barrier);
        let path = path.clone();
        tasks.push(tokio::spawn(async move {
            let stream = client
                .stream::<Record>(&path)
                .await
                .map_err(|error| error.to_string())?;
            tokio::pin!(stream);
            barrier.wait().await;
            let initial = next_event(&mut stream).await?;
            assert!(matches!(initial, TypedEvent::Put { data: None, .. }));
            for revision in 0..mutation_count {
                match next_event(&mut stream).await? {
                    TypedEvent::Put {
                        data: Some(data), ..
                    } => assert_eq!(data.revision, revision),
                    other => panic!("expected fan-out PUT, got {other:?}"),
                }
            }
            Ok::<u32, String>(mutation_count + 1)
        }));
    }

    barrier.wait().await;
    for revision in 0..mutation_count {
        let value = Record {
            stream: 0,
            revision,
            active: true,
        };
        let written: Record = client
            .put(&path, &value)
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(written, value);
    }

    let mut event_count = 0u32;
    for task in tasks {
        event_count += task.await.map_err(|error| error.to_string())??;
    }
    client
        .delete(&path)
        .await
        .map_err(|error| error.to_string())?;
    println!(
        "realtime fanout: subscribers={subscriber_count} mutations={mutation_count} events={event_count} parse_conversion_failures=0 missed_or_extra_events=0 elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(())
}

#[tokio::test]
#[ignore]
async fn emulator_realtime_filtered_child_and_close_contract() {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let root = format!("realtime-filtered/{run_id}");
    let client = emulator_client();
    let stream = client
        .query::<serde_json::Value>(&root)
        .order_by_key()
        .equal_to(rtdb_typed::rtdb_rs::FilterValue::string("one"))
        .stream()
        .await
        .unwrap();
    tokio::pin!(stream);
    assert!(matches!(
        next_event::<_, serde_json::Value>(&mut stream)
            .await
            .unwrap(),
        TypedEvent::Put {
            data: Some(data), ..
        } if data == serde_json::json!({})
    ));
    let value = Record {
        stream: 1,
        revision: 1,
        active: true,
    };
    let _: Record = client.put(&format!("{root}/one"), &value).await.unwrap();
    assert!(matches!(
        next_event::<_, serde_json::Value>(&mut stream).await.unwrap(),
        TypedEvent::Put {
            data: Some(data), ..
        } if data == serde_json::json!({"stream": 1, "revision": 1, "active": true})
    ));
    client.delete(&root).await.unwrap();
    let deleted = next_event::<_, serde_json::Value>(&mut stream)
        .await
        .unwrap();
    assert!(
        matches!(deleted, TypedEvent::Put { data: Some(ref data), .. } if data == &serde_json::json!({})),
        "unexpected filtered delete event: {deleted:?}"
    );
}
