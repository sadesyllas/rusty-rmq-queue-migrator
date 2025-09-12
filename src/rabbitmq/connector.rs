use crate::error::AppError;
use lapin::{Connection, ConnectionProperties};
use urlencoding::encode;

/// Establishes a connection to a RabbitMQ broker.
pub async fn connect(
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
    vhost: &str,
) -> Result<Connection, AppError> {
    let encoded_vhost = encode(vhost);
    let addr = format!("amqp://{}:{}@{}:{}/{}", user, pass, host, port, encoded_vhost);
    let conn = Connection::connect(&addr, ConnectionProperties::default()).await?;
    Ok(conn)
}
