extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::net::socket::listen::Listen;
use crate::net::socket::{TCPBoundSocket, TCPSocket};
use crate::net::tx::NetworkError;

pub struct HTTPServer {
    socket: TCPBoundSocket,
}

impl HTTPServer {
    pub fn new(listen: impl Into<Listen>) -> Self {
        Self {
            socket: TCPSocket::listen(listen),
        }
    }

    pub async fn accept_loop(&self) -> Result<(), NetworkError> {
        let connection = self.socket.accept().await;

        let request = connection.receive().await.map_err(NetworkError::Tcp)?;
        let request = String::from_utf8_lossy(&request);

        klog!("http_server", "New request: ", request);

        let response = ResponseBuilder::default()
            .add_header("Server", "RustKernel")
            .add_header("Content-Type", "text/html")
            .set_content_string("<h1>Welcome to RustKernel!</h1>\n")
            .build();

        connection.send(&response).await?;

        Ok(())
    }
}

#[derive(Debug, Default)]
struct ResponseBuilder {
    headers: BTreeMap<String, String>,
    content: Option<Vec<u8>>,
}

impl ResponseBuilder {
    fn add_header(mut self, header: impl Into<String>, content: impl Into<String>) -> Self {
        self.headers.insert(header.into(), content.into());
        self
    }

    fn set_content_string(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into().as_bytes().to_vec());
        let content_size = self
            .content
            .as_ref()
            .map(|content| content.len())
            .unwrap_or_default();
        self.add_header("Content-Size", format!("{content_size}"))
    }

    fn build(self) -> Vec<u8> {
        let mut response = "HTTP/1.1 200 OK\r\n".as_bytes().to_vec();

        self.headers
            .iter()
            .map(|(header, content)| format!("{header}: {content}\r\n"))
            .for_each(|line| response.extend(line.as_bytes()));

        response.extend("\r\n".as_bytes());

        response.extend(self.content.unwrap_or_default());

        response
    }
}
