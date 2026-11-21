//! Le protocole d'exécution distante (annexe K) : les trames de K.2, les messages JSON de K.3, le serveur et le client.
#![forbid(unsafe_code)]

pub mod client;
pub mod frame;
pub mod message;
pub mod server;
