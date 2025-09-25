// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

//! Custom codec for telnet connections supporting both text and binary modes
//! with explicit flush control, similar to LambdaMOO's networking capabilities.

use bytes::{Buf, Bytes, BytesMut};
use std::fmt;
use std::io;
use tokio_util::codec::{Decoder, Encoder};

/// Connection mode determines how data is parsed and handled
#[derive(Copy, Debug, Clone, PartialEq, Eq)]
pub enum ConnectionMode {
    /// Text mode: parse input into lines, handle line endings
    Text,
    /// Binary mode: pass through raw bytes without processing
    Binary,
}

/// Items emitted by the decoder based on connection mode
#[derive(Debug)]
pub enum ConnectionItem {
    /// A complete line (text mode only)
    Line(String),
    /// Raw bytes (binary mode, or partial data in text mode)
    Bytes(#[allow(dead_code)] Bytes),
}

/// Frames that can be encoded and sent
#[derive(Debug)]
pub enum ConnectionFrame {
    /// Send a line with automatic newline appending
    Line(String),
    /// Send raw text without adding newline (for no_newline attribute)
    RawText(String),
    /// Send raw bytes without modification
    Bytes(Bytes),
    /// Explicit flush command
    Flush,
    /// Switch codec mode (text vs binary)
    SetMode(ConnectionMode),
}

/// Errors that can occur during codec operations
#[derive(Debug)]
pub enum ConnectionCodecError {
    /// Line exceeded maximum length in text mode
    MaxLineLengthExceeded,
    /// IO error occurred
    Io(io::Error),
    /// UTF-8 decoding error in text mode
    Utf8(std::str::Utf8Error),
}

impl fmt::Display for ConnectionCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionCodecError::MaxLineLengthExceeded => {
                write!(f, "maximum line length exceeded")
            }
            ConnectionCodecError::Io(e) => write!(f, "IO error: {e}"),
            ConnectionCodecError::Utf8(e) => write!(f, "UTF-8 error: {e}"),
        }
    }
}

impl std::error::Error for ConnectionCodecError {}

impl From<io::Error> for ConnectionCodecError {
    fn from(e: io::Error) -> Self {
        ConnectionCodecError::Io(e)
    }
}

impl From<std::str::Utf8Error> for ConnectionCodecError {
    fn from(e: std::str::Utf8Error) -> Self {
        ConnectionCodecError::Utf8(e)
    }
}

/// Custom codec supporting both text and binary modes with explicit flush control
pub struct ConnectionCodec {
    mode: ConnectionMode,
    // Text mode parsing state (similar to LinesCodec)
    next_index: usize,
    max_length: Option<usize>,
    is_discarding: bool,
}

impl ConnectionCodec {
    /// Create a new codec in text mode without line length limits
    pub fn new() -> Self {
        Self {
            mode: ConnectionMode::Text,
            next_index: 0,
            max_length: None,
            is_discarding: false,
        }
    }

    /// Create a new codec in text mode with maximum line length
    #[allow(dead_code)]
    pub fn new_with_max_length(max_length: usize) -> Self {
        Self {
            mode: ConnectionMode::Text,
            next_index: 0,
            max_length: Some(max_length),
            is_discarding: false,
        }
    }

    /// Create a new codec in binary mode
    #[allow(dead_code)]
    pub fn new_binary() -> Self {
        Self {
            mode: ConnectionMode::Binary,
            next_index: 0,
            max_length: None,
            is_discarding: false,
        }
    }

    /// Get current connection mode
    #[allow(dead_code)]
    pub fn mode(&self) -> &ConnectionMode {
        &self.mode
    }

    /// Set connection mode
    pub fn set_mode(&mut self, mode: ConnectionMode) {
        self.mode = mode;
        // Reset text mode state when switching modes
        if mode == ConnectionMode::Binary {
            self.next_index = 0;
            self.is_discarding = false;
        }
    }

    /// Decode a line from buffer (text mode logic adapted from LinesCodec)
    fn decode_line(&mut self, buf: &mut BytesMut) -> Result<Option<String>, ConnectionCodecError> {
        let read_to = buf.len();

        // Look for newline character
        let Some(newline_offset) = buf[self.next_index..read_to]
            .iter()
            .position(|b| *b == b'\n')
        else {
            // No newline found, check length limits and exit
            return self.handle_no_newline_found(buf, read_to);
        };

        let newline_index = newline_offset + self.next_index;

        // Check line length limit
        if let Some(max_length) = self.max_length
            && newline_index > max_length {
                return Err(ConnectionCodecError::MaxLineLengthExceeded);
            }

        // Extract and process the line
        let mut line = buf.split_to(newline_index + 1);
        line.truncate(newline_index); // Remove the newline

        // Remove trailing carriage return if present
        if line.ends_with(b"\r") {
            line.truncate(line.len() - 1);
        }

        // Reset parsing state
        self.next_index = 0;
        self.is_discarding = false;

        // Convert to string and return
        let line_str = std::str::from_utf8(&line)?.to_string();
        Ok(Some(line_str))
    }

    /// Handle the case where no newline was found
    fn handle_no_newline_found(
        &mut self,
        buf: &mut BytesMut,
        read_to: usize,
    ) -> Result<Option<String>, ConnectionCodecError> {
        let Some(max_length) = self.max_length else {
            // No length limit, just wait for more data
            self.next_index = read_to;
            return Ok(None);
        };

        if read_to <= max_length {
            // Under limit, wait for more data
            self.next_index = read_to;
            return Ok(None);
        }

        // Over limit - handle discarding logic
        if self.is_discarding {
            // Already discarding, continue until newline
            let Some(newline_offset) = buf.iter().position(|b| *b == b'\n') else {
                // No newline yet, discard all and wait
                buf.advance(read_to);
                return Ok(None);
            };

            // Found newline, discard up to it and reset
            buf.advance(newline_offset + 1);
            self.is_discarding = false;
            self.next_index = 0;
            return Ok(None);
        }

        // First time hitting limit, start discarding
        self.is_discarding = true;
        Err(ConnectionCodecError::MaxLineLengthExceeded)
    }
}

impl Default for ConnectionCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder for ConnectionCodec {
    type Item = ConnectionItem;
    type Error = ConnectionCodecError;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if buf.is_empty() {
            return Ok(None);
        }

        match self.mode {
            ConnectionMode::Text => {
                // Parse lines using LinesCodec-style logic
                let Some(line) = self.decode_line(buf)? else {
                    return Ok(None);
                };
                Ok(Some(ConnectionItem::Line(line)))
            }
            ConnectionMode::Binary => {
                // In binary mode, pass through all available bytes
                let bytes = buf.split().freeze();
                Ok(Some(ConnectionItem::Bytes(bytes)))
            }
        }
    }
}

impl Encoder<ConnectionFrame> for ConnectionCodec {
    type Error = ConnectionCodecError;

    fn encode(&mut self, frame: ConnectionFrame, buf: &mut BytesMut) -> Result<(), Self::Error> {
        match frame {
            ConnectionFrame::Line(line) => {
                // Add the line and append a newline
                buf.extend_from_slice(line.as_bytes());
                buf.extend_from_slice(b"\n");
            }
            ConnectionFrame::RawText(text) => {
                // Add raw text without newline (for no_newline attribute)
                buf.extend_from_slice(text.as_bytes());
            }
            ConnectionFrame::Bytes(bytes) => {
                // Add raw bytes without modification
                buf.extend_from_slice(&bytes);
            }
            ConnectionFrame::Flush => {
                // Flush is a no-op for encoding - the framing layer handles actual flushing
                // This frame type is used to signal when a flush should occur
            }
            ConnectionFrame::SetMode(mode) => {
                // Switch the codec mode
                self.set_mode(mode);
                // No data is written to the buffer for mode changes
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn test_text_mode_line_parsing() {
        let mut codec = ConnectionCodec::new();
        let mut buf = BytesMut::from("hello\nworld\r\n");

        // First line
        let item = codec.decode(&mut buf).unwrap().unwrap();
        match item {
            ConnectionItem::Line(line) => assert_eq!(line, "hello"),
            _ => panic!("Expected line"),
        }

        // Second line (with CRLF)
        let item = codec.decode(&mut buf).unwrap().unwrap();
        match item {
            ConnectionItem::Line(line) => assert_eq!(line, "world"),
            _ => panic!("Expected line"),
        }

        // No more data
        assert!(codec.decode(&mut buf).unwrap().is_none());
    }

    #[test]
    fn test_binary_mode() {
        let mut codec = ConnectionCodec::new_binary();
        let test_data = b"hello\nworld\x00\xff";
        let mut buf = BytesMut::from(&test_data[..]);

        let item = codec.decode(&mut buf).unwrap().unwrap();
        match item {
            ConnectionItem::Bytes(bytes) => assert_eq!(bytes, &test_data[..]),
            _ => panic!("Expected bytes"),
        }
    }

    #[test]
    fn test_encoding_line() {
        let mut codec = ConnectionCodec::new();
        let mut buf = BytesMut::new();

        codec
            .encode(ConnectionFrame::Line("test".to_string()), &mut buf)
            .unwrap();
        assert_eq!(buf, "test\n");
    }

    #[test]
    fn test_encoding_raw_text() {
        let mut codec = ConnectionCodec::new();
        let mut buf = BytesMut::new();

        codec
            .encode(ConnectionFrame::RawText("no newline".to_string()), &mut buf)
            .unwrap();
        assert_eq!(buf, "no newline");
    }

    #[test]
    fn test_encoding_bytes() {
        let mut codec = ConnectionCodec::new();
        let mut buf = BytesMut::new();
        let test_bytes = Bytes::from_static(b"raw\x00data");

        codec
            .encode(ConnectionFrame::Bytes(test_bytes.clone()), &mut buf)
            .unwrap();
        assert_eq!(buf, test_bytes);
    }

    #[test]
    fn test_max_line_length() {
        let mut codec = ConnectionCodec::new_with_max_length(5);
        let mut buf = BytesMut::from("toolong\n");

        let result = codec.decode(&mut buf);
        assert!(matches!(
            result,
            Err(ConnectionCodecError::MaxLineLengthExceeded)
        ));
    }

    #[test]
    fn test_mode_switching() {
        let mut codec = ConnectionCodec::new();
        assert_eq!(codec.mode(), &ConnectionMode::Text);

        codec.set_mode(ConnectionMode::Binary);
        assert_eq!(codec.mode(), &ConnectionMode::Binary);
    }

    #[test]
    fn test_encoding_set_mode() {
        let mut codec = ConnectionCodec::new();
        let mut buf = BytesMut::new();

        // Initially in text mode
        assert_eq!(codec.mode(), &ConnectionMode::Text);

        // Send SetMode frame to switch to binary
        codec
            .encode(ConnectionFrame::SetMode(ConnectionMode::Binary), &mut buf)
            .unwrap();

        // Should switch mode but not write any data to buffer
        assert_eq!(codec.mode(), &ConnectionMode::Binary);
        assert!(buf.is_empty());

        // Switch back to text mode
        codec
            .encode(ConnectionFrame::SetMode(ConnectionMode::Text), &mut buf)
            .unwrap();
        assert_eq!(codec.mode(), &ConnectionMode::Text);
        assert!(buf.is_empty());
    }
}
