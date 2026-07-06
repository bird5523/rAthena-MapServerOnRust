pub mod codec;
pub mod dispatcher;
pub mod inter_server;

use anyhow::Result;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use futures_util::{StreamExt, SinkExt};
use tracing::{info, debug, error};
use std::sync::Arc;
use crate::core::state::ServerState;
use codec::RoPacketCodec;
use dispatcher::Dispatcher;

pub async fn handle_connection(socket: TcpStream, state: Arc<ServerState>) -> Result<()> {
    // Wrap the TCP Stream with our Packet Codec
    let framed = Framed::new(socket, RoPacketCodec);
    let (mut sink, mut stream) = framed.split();
    
    let (packet_tx, mut packet_rx) = tokio::sync::mpsc::unbounded_channel::<crate::network::codec::RoPacket>();
    
    // Spawn a dedicated writer task for this connection
    tokio::spawn(async move {
        while let Some(packet) = packet_rx.recv().await {
            if let Err(e) = sink.send(packet).await {
                error!("Error sending packet to client: {:?}", e);
                break;
            }
        }
    });

    loop {
        match stream.next().await {
            Some(Ok(packet)) => {
                debug!("Received packet: {:?}", packet);
                // Dispatch packet handling
                if let Err(e) = Dispatcher::dispatch(packet, packet_tx.clone(), state.clone()).await {
                    error!("Error dispatching packet: {:?}", e);
                }
            }
            Some(Err(e)) => {
                error!("Error reading from socket: {:?}", e);
                break;
            }
            None => {
                info!("Client disconnected");
                break;
            }
        }
    }
    Ok(())
}
