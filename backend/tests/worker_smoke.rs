//! Real composition binary; exact database and broker resource ownership.
mod support;
use lapin::{Connection, ConnectionProperties, options::*};
use std::{process::Stdio, time::Duration};
use support::{checked, isolated};
use tokio::time::timeout;

#[tokio::test]
#[ignore = "requires PostgreSQL CREATE DATABASE and RabbitMQ"]
async fn compiled_worker_confirms_and_completes_then_sigterm_exits() {
    smoke(false).await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL CREATE DATABASE, RabbitMQ and Supervisor in app"]
async fn isolated_supervisor_named_and_collective_program_operation() {
    smoke(true).await;
}

async fn ctl(config: &std::path::Path, alias: &str, args: &[&str]) -> String {
    let mut command = tokio::process::Command::new(alias);
    command.arg("-c").arg(config).args(args).kill_on_drop(true);
    let output = timeout(Duration::from_secs(35), command.output())
        .await
        .unwrap()
        .unwrap();
    assert!(
        output.status.success() || (args[0] == "status" && output.status.code() == Some(3)),
        "Supervisor command failed"
    );
    String::from_utf8(output.stdout).unwrap()
}

async fn smoke(supervised: bool) {
    isolated(move |pool| async move {
        checked(sqlx::raw_sql(include_str!("../migrations/20261009000000_add_delivery_ownership.up.sql")).execute(&pool).await,"ownership migration");
        let event_id = uuid::Uuid::now_v7();
        checked(sqlx::query("INSERT INTO relay.outbox_events (id,event_type,aggregate_type,aggregate_id,schema_version,occurred_at,payload) VALUES ($1,'test','test','1',1,clock_timestamp(),'{\"smoke\":true}')").bind(event_id).execute(&pool).await,"seed test event");
        let name = format!("relay.it.{}",uuid::Uuid::now_v7().simple());
        let exchange = format!("{name}.exchange"); let queue = format!("{name}.queue");
        let mut uri=url::Url::parse("amqp://rabbitmq/relay").unwrap();
        uri.set_username(&std::env::var("RABBITMQ_DEFAULT_USER").unwrap()).unwrap();
        uri.set_password(Some(&std::env::var("RABBITMQ_DEFAULT_PASS").unwrap())).unwrap();
        uri.set_host(Some(&std::env::var("RABBITMQ_HOST").unwrap_or_else(|_|"rabbitmq".into()))).unwrap();
        uri.set_port(Some(std::env::var("RABBITMQ_PORT").unwrap_or_else(|_|"5672".into()).parse().unwrap())).unwrap();
        uri.path_segments_mut().unwrap().clear().push(&std::env::var("RABBITMQ_DEFAULT_VHOST").unwrap_or_else(|_|"relay".into()));
        let connection=timeout(Duration::from_secs(10),Connection::connect(uri.as_str(),ConnectionProperties::default())).await.unwrap().unwrap();
        let channel=connection.create_channel().await.unwrap();
        let database: String=checked(sqlx::query_scalar("SELECT current_database()").fetch_one(&pool).await,"isolated name");
        let temp=std::env::temp_dir().join(format!("relay_supervisor_{}",uuid::Uuid::now_v7().simple()));
        std::fs::create_dir(&temp).unwrap();
        let supervisor_config=temp.join("supervisord.conf");
        let physical=include_str!("../../docker/supervisor/normal.conf")
            .replace("/tmp/relay-supervisor",temp.to_str().unwrap())
            .replace("command=/usr/local/bin/relay-worker",&format!("command={}/../docker/supervisor/relay-worker",env!("CARGO_MANIFEST_DIR")));
        std::fs::write(&supervisor_config,physical).unwrap();
        let mut command=tokio::process::Command::new(if supervised {"/usr/bin/supervisord"} else {env!("CARGO_BIN_EXE_relay")});
        if supervised { command.arg("-c").arg(&supervisor_config); }
        let mut child=command.env("POSTGRES_DB",database).env("RABBITMQ_EXCHANGE",&exchange).env("RABBITMQ_QUEUE",&queue)
            .env("HTTP_ADDR","127.0.0.1:0").env("RELAY_IDLE_MS","10").stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true).spawn().unwrap();
        let case_pool=pool.clone(); let case_channel=channel.clone(); let case_queue=queue.clone();
        let case_config=supervisor_config.clone();
        let mut case_task=tokio::spawn(async move {
            if supervised {
                while !case_config.parent().unwrap().join("control.sock").exists() { tokio::time::sleep(Duration::from_millis(10)).await; }
                assert!(ctl(&case_config,"supervisorctl",&["status","relay"]).await.contains("STOPPED"));
                ctl(&case_config,"supervisorctl",&["start","relay"]).await;
            }
            loop {
                let processed: bool=checked(sqlx::query_scalar("SELECT status='processed' AND processed_at IS NOT NULL AND ownership_token IS NULL AND acquired_at IS NULL AND expires_at IS NULL AND attempt_count=1 FROM relay.outbox_events WHERE id=$1").bind(event_id).fetch_one(&case_pool).await,"completion observation");
                if processed { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            let message=case_channel.basic_get(case_queue.clone().into(),BasicGetOptions::default()).await.unwrap().expect("confirmed test message");
            let envelope: reliable_event_relay::domain::event::EventEnvelope=serde_json::from_slice(&message.data).unwrap();
            assert_eq!(envelope.id(),event_id); assert_eq!(envelope.payload(),&serde_json::json!({"smoke":true}));
            assert_eq!(message.properties.message_id().as_ref().unwrap().as_str(),event_id.to_string());
            message.ack(BasicAckOptions::default()).await.unwrap();
            if supervised {
                for alias in ["supervisorctl","supervisor"] {
                    for target in ["relay","http","all"] {
                        ctl(&case_config,alias,&["stop",target]).await;
                        ctl(&case_config,alias,&["start",target]).await;
                        ctl(&case_config,alias,&["restart",target]).await;
                    }
                    let status=ctl(&case_config,alias,&["status"]).await;
                    assert!(status.lines().filter(|line| line.contains("RUNNING")).count()==2);
                }
            }
        });
        let case=timeout(Duration::from_secs(30),&mut case_task).await;
        if case.is_err() { case_task.abort(); let _=case_task.await; }
        // Always kill/reap the exact child before database/broker cleanup.
        let signal=tokio::process::Command::new("kill").arg("-TERM").arg(child.id().expect("child PID").to_string()).status().await.unwrap();
        assert!(signal.success());
        let exit=timeout(Duration::from_secs(28),child.wait()).await;
        if exit.is_err() { child.kill().await.unwrap(); }
        timeout(Duration::from_secs(10),async {
            channel.queue_delete(queue.clone().into(),QueueDeleteOptions::default()).await.unwrap();
            channel.exchange_delete(exchange.clone().into(),ExchangeDeleteOptions::default()).await.unwrap();
            connection.close(200,"test cleanup".into()).await.unwrap();
        }).await.unwrap();
        std::fs::remove_dir_all(temp).unwrap();
        case.expect("composition completion deadline").expect("composition assertions"); assert!(exit.unwrap().unwrap().success());
        Ok(())
    }).await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL CREATE DATABASE"]
async fn missing_ownership_schema_fails_before_polling_with_sanitized_error() {
    isolated(|pool| async move {
        let database: String = checked(
            sqlx::query_scalar("SELECT current_database()")
                .fetch_one(&pool)
                .await,
            "isolated name",
        );
        let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_relay"));
        command.env("POSTGRES_DB", database).kill_on_drop(true);
        let output = timeout(Duration::from_secs(10), command.output())
            .await
            .unwrap()
            .unwrap();
        assert!(!output.status.success());
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostic.contains("required ownership schema unavailable"));
        assert!(!diagnostic.contains(&std::env::var("POSTGRES_PASSWORD").unwrap()));
        assert!(
            !String::from_utf8(output.stdout)
                .unwrap()
                .contains("publisher connecting")
        );
        let count: i64 = checked(
            sqlx::query_scalar("SELECT count(*) FROM relay.outbox_events")
                .fetch_one(&pool)
                .await,
            "no worker writes",
        );
        assert_eq!(count, 0);
        Ok(())
    })
    .await;
}
