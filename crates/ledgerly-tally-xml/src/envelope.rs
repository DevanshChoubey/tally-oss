//! Decoding and the envelope (UTF-8/UTF-16, invalid characters, truncation). **Card X1** implements this module.
//!
//! Frozen interface (plan card X1):
//!
//! ```text
//! pub struct Envelope { pub header: Header, pub messages: Vec<RawMessage> }
//! pub fn decode(bytes: &[u8]) -> Result<(String, Vec<XmlWarning>), TallyXmlError>;
//! pub fn parse_envelope(text: &str) -> Result<(Envelope, Vec<XmlWarning>), TallyXmlError>;
//! ```
