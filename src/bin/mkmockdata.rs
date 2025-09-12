use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use lapin::{
    options::*,
    types::{AMQPValue, FieldTable},
    BasicProperties,
};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use rusty_rmq_queue_migrator::{config::Config, rabbitmq::connector};
use std::collections::BTreeMap;

const NUM_QUEUES: u32 = 5;
const NUM_MESSAGES_PER_QUEUE: u32 = 100_000; // Reduced for faster testing

#[tokio::main]
async fn main() -> Result<()> {
    let mut config = Config::parse();

    if config.source_password.is_none() {
        config.source_password =
            Some(rpassword::prompt_password("Enter source RabbitMQ password: ")?);
    }
    if config.dest_password.is_none() {
        config.dest_password =
            Some(rpassword::prompt_password("Enter destination RabbitMQ password: ")?);
    }

    if let Err(e) = run(&config).await {
        eprintln!("\nMock data generation failed. See details below.");
        return Err(e);
    }

    println!("\nMock data generation complete!");
    Ok(())
}

async fn run(config: &Config) -> Result<()> {
    println!("Connecting to source RabbitMQ: {}", config.source_host);
    let source_conn = connector::connect(
        &config.source_host,
        config.source_port,
        &config.source_username,
        config.source_password.as_deref().unwrap_or(""),
        &config.source_vhost,
    )
    .await
    .context("Failed to connect to source RabbitMQ")?;
    println!("Successfully connected to source.");

    println!(
        "Connecting to destination RabbitMQ: {}",
        config.dest_host
    );
    let dest_conn = connector::connect(
        &config.dest_host,
        config.dest_port,
        &config.dest_username,
        config.dest_password.as_deref().unwrap_or(""),
        &config.dest_vhost,
    )
    .await
    .context("Failed to connect to destination RabbitMQ")?;
    println!("Successfully connected to destination.");

    let queue_names: Vec<String> = (1..=NUM_QUEUES).map(|i| format!("rmq-{}", i)).collect();

    let setup_result = async {
        generate_all_data(&source_conn, &queue_names).await?;
        create_empty_queues(&dest_conn, &queue_names).await
    }
    .await;

    if setup_result.is_err() {
        eprintln!("\nAn error occurred during mock data setup. Attempting cleanup...");
        if let Ok(channel) = source_conn.create_channel().await {
            println!("Cleaning up source queues...");
            for name in &queue_names {
                let _ = channel.queue_delete(name, Default::default()).await;
            }
        }
        if let Ok(channel) = dest_conn.create_channel().await {
            println!("Cleaning up destination queues...");
            for name in &queue_names {
                let _ = channel.queue_delete(name, Default::default()).await;
            }
        }
    }

    setup_result
}

async fn generate_all_data(
    conn: &lapin::Connection,
    queue_names: &[String],
) -> Result<()> {
    println!("\nGenerating mock data on source...");
    for queue_name in queue_names {
        let pb = ProgressBar::new(NUM_MESSAGES_PER_QUEUE as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} ({eta}) {msg}",
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        pb.set_message(queue_name.clone());

        create_and_populate_queue(conn, queue_name.clone(), pb).await?;
    }
    Ok(())
}

async fn create_and_populate_queue(
    conn: &lapin::Connection,
    queue_name: String,
    pb: ProgressBar,
) -> Result<()> {
    let channel = conn.create_channel().await?;
    channel
        .queue_declare(
            &queue_name,
            QueueDeclareOptions {
                durable: false,
                exclusive: false,
                auto_delete: true, // Auto-delete when all consumers disconnect
                ..Default::default()
            },
            Default::default(),
        )
        .await?;

    let mut rng = thread_rng();
    for _ in 0..NUM_MESSAGES_PER_QUEUE {
        let payload = generate_random_payload(&mut rng, 200, 1024);
        let headers = generate_random_headers(&mut rng);
        let props = BasicProperties::default().with_headers(headers);

        channel
            .basic_publish("", &queue_name, BasicPublishOptions::default(), &payload, props)
            .await?;
        pb.inc(1);
    }
    pb.finish_with_message(format!("Finished {}", queue_name));
    Ok(())
}

async fn create_empty_queues(conn: &lapin::Connection, queue_names: &[String]) -> Result<()> {
    println!("\nCreating empty queues on destination...");
    let channel = conn.create_channel().await?;
    for queue_name in queue_names {
        channel
            .queue_declare(
                queue_name,
                QueueDeclareOptions {
                    durable: false,
                    exclusive: false,
                    auto_delete: true, // Auto-delete when all consumers disconnect
                    ..Default::default()
                },
                Default::default(),
            )
            .await?;
        println!("- Created destination queue: {}", queue_name);
    }
    channel.close(200, "work complete").await?;
    println!("Destination queues created.");
    Ok(())
}

fn generate_random_payload(rng: &mut impl Rng, min: usize, max: usize) -> Vec<u8> {
    let len = rng.gen_range(min..=max);
    let mut payload = vec![0u8; len];
    rng.fill_bytes(&mut payload);
    payload
}

fn generate_random_headers(rng: &mut impl Rng) -> FieldTable {
    let mut headers = BTreeMap::new();
    let num_headers = rng.gen_range(1..=3);
    for _ in 0..num_headers {
        let key_len = rng.gen_range(5..=10);
        let key: String = rng.sample_iter(&Alphanumeric).take(key_len).map(char::from).collect();

        let val_len = rng.gen_range(10..=20);
        let value: String = rng.sample_iter(&Alphanumeric).take(val_len).map(char::from).collect();

        headers.insert(key.into(), AMQPValue::LongString(value.into()));
    }
    headers.into()
}
