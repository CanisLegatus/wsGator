use async_trait::async_trait;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async, tungstenite::Error as WsError, MaybeTlsStream, WebSocketStream,
};

// Runner 
// Algorithm of a load 
// Creation and management of connection pool 
// Passing params to ClientContext


#[async_trait]
pub trait Runner: Send + Sync {
    async fn get_ws_connection(
        &self,
        url: &str,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, WsError> {
        let (ws, _) = connect_async(url).await?;
        Ok(ws)
    }

    async fn run(&self);
}

// Structs

pub struct LinearRunner {}

pub struct RampUpRunner {}

// Implementations

#[async_trait]
impl Runner for LinearRunner {
    async fn run(&self) {}
}

#[async_trait]
impl Runner for RampUpRunner {
    async fn run(&self) {}
}
// Getting run logic
//            let runner = strategy
//              .clone()
//            .prepare_strategy(config.clone(), log.clone())
//           .await?;
//
//          println!("---> Wave ATTACK stage...");
//
// Running tasks and collecting them to join_set
//          let mut join_set = runner.await?;

//        // Handling errors in async tasks
//      while let Some(result_of_async_task) = join_set.join_next().await {
//        match result_of_async_task {
//          Ok(Ok(())) => continue,
//         Ok(Err(e)) => {
//           log.count(e);
//     }
//   Err(join_error) => {
//     println!("Join Error! Error: {join_error}");
//}
//}
//}
