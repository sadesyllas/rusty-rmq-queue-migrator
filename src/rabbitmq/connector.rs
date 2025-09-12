use crate::error::AppError;
use lapin::{Connection, ConnectionProperties};

/// Establishes a connection to a RabbitMQ broker.
pub async fn connect(
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
) -> Result<Connection, AppError> {
    let addr = format!("amqp://{}:{}@{}:{}", user, pass, host, port);
    let conn = Connection::connect(&addr, ConnectionProperties::default()).await?;
    Ok(conn)
}
