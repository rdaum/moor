// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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
//!
//! Telnet protocol commands (IAC sequences) are parsed using a state machine
//! modeled after ToastStunt's `process_telnet_byte` in network.cc. Commands
//! are emitted as [`ConnectionItem::TelnetCommand`] for out-of-band processing
//! rather than being silently discarded.

use bytes::{Buf, Bytes, BytesMut};
use std::{fmt, io};
use tokio_util::codec::{Decoder, Encoder};

/// Connection mode determines how data is parsed and handled
#[derive(Copy, Debug, Clone, PartialEq, Eq)]
pub enum ConnectionMode {
    /// Text mode: parse input into lines, handle line endings
    Text,
    /// Binary mode: pass through raw bytes without processing
    Binary,
}

/// Telnet protocol parsing state, modeled after ToastStunt's TelnetState enum.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum TelnetState {
    /// Processing normal text input
    Normal,
    /// Just saw IAC byte (0xFF)
    Iac,
    /// Reading option byte after WILL/WONT/DO/DONT
    WillWontDoDont,
    /// Inside subnegotiation (after IAC SB)
    Subneg,
    /// Saw IAC while inside subnegotiation
    SubnegIac,
}

/// Items emitted by the decoder based on connection mode
#[derive(Debug)]
pub enum ConnectionItem {
    /// A complete line (text mode only)
    Line(String),
    /// Raw bytes (binary mode)
    Bytes(Bytes),
    /// A complete telnet command sequence (IAC + command bytes).
    /// Emitted as out-of-band data for the connection to handle.
    TelnetCommand(Bytes),
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
}

impl fmt::Display for ConnectionCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionCodecError::MaxLineLengthExceeded => {
                write!(f, "maximum line length exceeded")
            }
            ConnectionCodecError::Io(e) => write!(f, "IO error: {e}"),
        }
    }
}

impl std::error::Error for ConnectionCodecError {}

impl From<io::Error> for ConnectionCodecError {
    fn from(e: io::Error) -> Self {
        ConnectionCodecError::Io(e)
    }
}

/// Custom codec supporting both text and binary modes with telnet protocol
/// handling and explicit flush control, similar to LambdaMOO/ToastStunt.
pub struct ConnectionCodec {
    mode: ConnectionMode,
    /// Telnet protocol parsing state (text mode only)
    telnet_state: TelnetState,
    /// Accumulates text bytes for the current line being parsed
    line_buf: Vec<u8>,
    /// Accumulates bytes for the current telnet command sequence
    command_buf: Vec<u8>,
    /// Maximum allowed line length (None for unlimited)
    max_length: Option<usize>,
    /// Track CR state for proper CRLF handling like LambdaMOO
    last_input_was_cr: bool,
}

impl ConnectionCodec {
    /// Create a new codec in text mode without line length limits
    pub fn new() -> Self {
        Self {
            mode: ConnectionMode::Text,
            telnet_state: TelnetState::Normal,
            line_buf: Vec::new(),
            command_buf: Vec::new(),
            max_length: None,
            last_input_was_cr: false,
        }
    }

    /// Create a new codec in text mode with maximum line length
    #[cfg(test)]
    pub fn new_with_max_length(max_length: usize) -> Self {
        Self {
            max_length: Some(max_length),
            ..Self::new()
        }
    }

    /// Create a new codec in binary mode
    #[cfg(test)]
    pub fn new_binary() -> Self {
        Self {
            mode: ConnectionMode::Binary,
            ..Self::new()
        }
    }

    /// Get current connection mode
    #[cfg(test)]
    pub fn mode(&self) -> &ConnectionMode {
        &self.mode
    }

    /// Set connection mode
    pub fn set_mode(&mut self, mode: ConnectionMode) {
        self.mode = mode;
        if mode == ConnectionMode::Binary {
            self.telnet_state = TelnetState::Normal;
            self.command_buf.clear();
            // Preserve last_input_was_cr across mode switches to handle pending LF from CRLF
        }
    }

    /// Decode text mode input using a byte-by-byte telnet state machine.
    ///
    /// This mirrors ToastStunt's `process_telnet_byte()` function: each byte is
    /// routed through the state machine. Text bytes accumulate in `line_buf`;
    /// telnet command bytes accumulate in `command_buf`. The function returns as
    /// soon as a complete item (line or telnet command) is available.
    fn decode_text(
        &mut self,
        buf: &mut BytesMut,
    ) -> Result<Option<ConnectionItem>, ConnectionCodecError> {
        while !buf.is_empty() {
            let c = buf[0];
            buf.advance(1);

            match self.telnet_state {
                TelnetState::Normal => {
                    if c == 0xFF {
                        // IAC — begin telnet command sequence
                        self.telnet_state = TelnetState::Iac;
                        self.command_buf.clear();
                        self.command_buf.push(c);
                    } else if c == b'\r' || (c == b'\n' && !self.last_input_was_cr) {
                        // Line ending (LambdaMOO semantics)
                        self.last_input_was_cr = c == b'\r';
                        let line = String::from_utf8_lossy(&self.line_buf).into_owned();
                        self.line_buf.clear();
                        return Ok(Some(ConnectionItem::Line(line)));
                    } else if c == b'\n' && self.last_input_was_cr {
                        // LF immediately following CR — part of CRLF, ignore
                        self.last_input_was_cr = false;
                    } else {
                        self.last_input_was_cr = false;
                        // Keep printable characters and tab, filter control chars
                        // Matches ToastStunt: isgraph(c) || c == ' ' || c == '\t'
                        if c == 0x09 || (c >= 0x20 && c != 0x7F) {
                            self.line_buf.push(c);
                            // Check max line length
                            if let Some(max) = self.max_length
                                && self.line_buf.len() > max
                            {
                                self.line_buf.clear();
                                return Err(ConnectionCodecError::MaxLineLengthExceeded);
                            }
                        }
                    }
                }
                TelnetState::Iac => {
                    self.command_buf.push(c);
                    match c {
                        0xFF => {
                            // IAC IAC — escaped literal 0xFF
                            // Not valid in UTF-8 text, so just discard
                            self.telnet_state = TelnetState::Normal;
                        }
                        0xFA => {
                            // SB — subnegotiation begin
                            self.telnet_state = TelnetState::Subneg;
                        }
                        0xFB..=0xFE => {
                            // WILL (0xFB), WONT (0xFC), DO (0xFD), DONT (0xFE)
                            // Need one more byte (the option code)
                            self.telnet_state = TelnetState::WillWontDoDont;
                        }
                        _ => {
                            // Two-byte command: NOP(0xF1), DM(0xF2), BRK(0xF3),
                            // IP(0xF4), AO(0xF5), AYT(0xF6), EC(0xF7), EL(0xF8),
                            // GA(0xF9), SE(0xF0), or unknown
                            self.telnet_state = TelnetState::Normal;
                            let cmd = Bytes::from(std::mem::take(&mut self.command_buf));
                            return Ok(Some(ConnectionItem::TelnetCommand(cmd)));
                        }
                    }
                }
                TelnetState::WillWontDoDont => {
                    // Option byte after WILL/WONT/DO/DONT
                    self.command_buf.push(c);
                    self.telnet_state = TelnetState::Normal;
                    let cmd = Bytes::from(std::mem::take(&mut self.command_buf));
                    return Ok(Some(ConnectionItem::TelnetCommand(cmd)));
                }
                TelnetState::Subneg => {
                    self.command_buf.push(c);
                    if c == 0xFF {
                        self.telnet_state = TelnetState::SubnegIac;
                    }
                }
                TelnetState::SubnegIac => {
                    self.command_buf.push(c);
                    match c {
                        0xF0 => {
                            // SE — end of subnegotiation
                            self.telnet_state = TelnetState::Normal;
                            let cmd = Bytes::from(std::mem::take(&mut self.command_buf));
                            return Ok(Some(ConnectionItem::TelnetCommand(cmd)));
                        }
                        0xFF => {
                            // IAC IAC within subneg — escaped 0xFF data byte
                            self.telnet_state = TelnetState::Subneg;
                        }
                        _ => {
                            // Unexpected byte after IAC in subneg — continue
                            self.telnet_state = TelnetState::Subneg;
                        }
                    }
                }
            }
        }

        Ok(None)
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
            ConnectionMode::Text => self.decode_text(buf),
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
                buf.extend_from_slice(line.as_bytes());
                buf.extend_from_slice(b"\r\n"); // telnet protocol requires CRLF
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

    /// Helper: collect all items from a single decode pass until None
    fn decode_all(codec: &mut ConnectionCodec, buf: &mut BytesMut) -> Vec<ConnectionItem> {
        let mut items = Vec::new();
        while let Some(item) = codec.decode(buf).unwrap() {
            items.push(item);
        }
        items
    }

    /// Helper: extract line text, panicking if not a Line item
    fn expect_line(item: ConnectionItem) -> String {
        match item {
            ConnectionItem::Line(line) => line,
            other => panic!("Expected Line, got {:?}", other),
        }
    }

    /// Helper: extract telnet command bytes, panicking if not a TelnetCommand item
    fn expect_telnet_cmd(item: ConnectionItem) -> Bytes {
        match item {
            ConnectionItem::TelnetCommand(cmd) => cmd,
            other => panic!("Expected TelnetCommand, got {:?}", other),
        }
    }

    #[test]
    fn test_text_mode_line_parsing() {
        let mut codec = ConnectionCodec::new();
        let mut buf = BytesMut::from("hello\nworld\r\n");

        let items = decode_all(&mut codec, &mut buf);
        assert_eq!(items.len(), 2);
        assert_eq!(expect_line(items.into_iter().next().unwrap()), "hello");
    }

    #[test]
    fn test_text_mode_line_parsing_both() {
        let mut codec = ConnectionCodec::new();
        let mut buf = BytesMut::from("hello\nworld\r\n");

        // First line (LF ending)
        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "hello");

        // Second line (CRLF ending - CR triggers completion, LF ignored)
        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "world");

        // No more data
        assert!(codec.decode(&mut buf).unwrap().is_none());
    }

    #[test]
    fn test_lambdamoo_cr_lf_handling() {
        let mut codec = ConnectionCodec::new();

        // Test standalone CR
        let mut buf = BytesMut::from("line1\r");
        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "line1");

        // Test LF after CR should be ignored, then parse next line
        let mut buf = BytesMut::from("\nline2\n");
        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "line2");

        // Test standalone LF (not after CR)
        let mut buf = BytesMut::from("line3\n");
        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "line3");

        // Test CRLF sequence
        let mut buf = BytesMut::from("line4\r\nline5\n");

        // CR should trigger line completion
        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "line4");

        // LF after CR should be ignored, then next line should work normally
        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "line5");
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
        assert_eq!(buf, "test\r\n");
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

    #[test]
    fn test_non_utf8_handling() {
        let mut codec = ConnectionCodec::new();

        // Create buffer with valid ASCII, then invalid UTF-8, then more ASCII
        // 0xC0 followed by ASCII is invalid (incomplete sequence)
        let mut buf = BytesMut::from(&b"hello \xC0 world\n"[..]);

        let item = codec.decode(&mut buf).unwrap().unwrap();
        match item {
            ConnectionItem::Line(line) => {
                // Invalid byte should be replaced with U+FFFD (replacement character)
                assert!(line.contains('\u{FFFD}'));
                assert!(line.starts_with("hello"));
                assert!(line.ends_with("world"));
            }
            _ => panic!("Expected line"),
        }
    }

    #[test]
    fn test_completely_invalid_utf8() {
        let mut codec = ConnectionCodec::new();

        // Use incomplete multi-byte sequences that each produce a replacement char
        // \xC0 is an invalid UTF-8 start byte (overlong encoding)
        let mut buf = BytesMut::from(&b"\xC0a\xC0b\xC0c\n"[..]);

        let item = codec.decode(&mut buf).unwrap().unwrap();
        match item {
            ConnectionItem::Line(line) => {
                // Each \xC0 is an invalid start byte, producing replacement characters
                assert_eq!(line.chars().filter(|&c| c == '\u{FFFD}').count(), 3);
                assert!(line.contains('a') && line.contains('b') && line.contains('c'));
            }
            _ => panic!("Expected line"),
        }
    }

    #[test]
    fn test_utf8_cjk_characters() {
        let mut codec = ConnectionCodec::new();

        // Test CJK characters that have continuation bytes in 0x80-0x9F range
        // 写 = E5 86 99 (0x86 and 0x99 are in the 0x80-0x9F range)
        let test_str = "读写汉字 - 学中文";
        let mut buf = BytesMut::from(format!("{}\n", test_str).as_bytes());

        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), test_str);
    }

    // --- Telnet protocol tests ---

    #[test]
    fn test_telnet_nop_emitted_as_command() {
        let mut codec = ConnectionCodec::new();

        // IAC NOP (0xFF 0xF1) should be emitted as a TelnetCommand
        let mut buf = BytesMut::from(&b"\xFF\xF1hello\n"[..]);

        let item = codec.decode(&mut buf).unwrap().unwrap();
        let cmd = expect_telnet_cmd(item);
        assert_eq!(cmd.as_ref(), &[0xFF, 0xF1]);

        // Text after the NOP should be a normal line
        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "hello");
    }

    #[test]
    fn test_telnet_nop_standalone() {
        let mut codec = ConnectionCodec::new();

        // IAC NOP with no text — should emit command, buffer empty after
        let mut buf = BytesMut::from(&b"\xFF\xF1"[..]);
        let item = codec.decode(&mut buf).unwrap().unwrap();
        let cmd = expect_telnet_cmd(item);
        assert_eq!(cmd.as_ref(), &[0xFF, 0xF1]);
        assert!(buf.is_empty());

        // Subsequent text should arrive clean
        let mut buf = BytesMut::from(&b"hello\n"[..]);
        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "hello");
    }

    #[test]
    fn test_telnet_nop_between_lines() {
        let mut codec = ConnectionCodec::new();

        // NOP arriving between two lines of text
        let mut buf = BytesMut::from(&b"line1\r\n\xFF\xF1line2\r\n"[..]);

        let items = decode_all(&mut codec, &mut buf);
        assert_eq!(items.len(), 3); // line1, NOP command, line2

        assert_eq!(expect_line(items.into_iter().next().unwrap()), "line1");
    }

    #[test]
    fn test_telnet_nop_mid_line() {
        let mut codec = ConnectionCodec::new();

        // NOP in the middle of text — text on both sides should join into one line
        let mut buf = BytesMut::from(&b"hel\xFF\xF1lo\n"[..]);

        // First item: the NOP command (emitted when the state machine completes it)
        // But text before/after NOP accumulates in line_buf across the command
        let item = codec.decode(&mut buf).unwrap().unwrap();
        let cmd = expect_telnet_cmd(item);
        assert_eq!(cmd.as_ref(), &[0xFF, 0xF1]);

        // Second item: the complete line with text from both sides of NOP
        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "hello");
    }

    #[test]
    fn test_telnet_will_echo() {
        let mut codec = ConnectionCodec::new();

        // IAC WILL ECHO (3-byte sequence)
        let mut buf = BytesMut::from(&b"\xFF\xFB\x01hello\n"[..]);

        let item = codec.decode(&mut buf).unwrap().unwrap();
        let cmd = expect_telnet_cmd(item);
        assert_eq!(cmd.as_ref(), &[0xFF, 0xFB, 0x01]); // IAC WILL ECHO

        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "hello");
    }

    #[test]
    fn test_telnet_do_dont() {
        let mut codec = ConnectionCodec::new();

        // IAC DO NAWS (0xFF 0xFD 0x1F) followed by IAC DONT ECHO (0xFF 0xFE 0x01)
        let mut buf = BytesMut::from(&b"\xFF\xFD\x1F\xFF\xFE\x01ok\n"[..]);

        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_telnet_cmd(item).as_ref(), &[0xFF, 0xFD, 0x1F]);

        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_telnet_cmd(item).as_ref(), &[0xFF, 0xFE, 0x01]);

        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "ok");
    }

    #[test]
    fn test_telnet_subnegotiation() {
        let mut codec = ConnectionCodec::new();

        // IAC SB NAWS <width_hi> <width_lo> <height_hi> <height_lo> IAC SE
        let mut buf = BytesMut::from(&b"\xFF\xFA\x1F\x00\x50\x00\x18\xFF\xF0hello\n"[..]);

        let item = codec.decode(&mut buf).unwrap().unwrap();
        let cmd = expect_telnet_cmd(item);
        assert_eq!(
            cmd.as_ref(),
            &[0xFF, 0xFA, 0x1F, 0x00, 0x50, 0x00, 0x18, 0xFF, 0xF0]
        );

        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "hello");
    }

    #[test]
    fn test_telnet_iac_iac_escape() {
        let mut codec = ConnectionCodec::new();

        // IAC IAC = escaped literal 0xFF — discarded since 0xFF isn't valid text
        let mut buf = BytesMut::from(&b"hello\xFF\xFFworld\n"[..]);

        let item = codec.decode(&mut buf).unwrap().unwrap();
        // IAC IAC doesn't emit a command, text on both sides joins
        assert_eq!(expect_line(item), "helloworld");
    }

    #[test]
    fn test_telnet_multiple_nops() {
        let mut codec = ConnectionCodec::new();

        // Multiple IAC NOP sequences then text
        let mut buf = BytesMut::from(&b"\xFF\xF1\xFF\xF1say hello\n"[..]);

        let items = decode_all(&mut codec, &mut buf);
        assert_eq!(items.len(), 3); // NOP, NOP, line

        assert_eq!(
            expect_telnet_cmd(items.into_iter().next().unwrap()).as_ref(),
            &[0xFF, 0xF1]
        );
    }

    #[test]
    fn test_telnet_incomplete_iac_at_end() {
        let mut codec = ConnectionCodec::new();

        // Lone IAC at end of buffer — state preserved for next decode
        let mut buf = BytesMut::from(&b"hello\xFF"[..]);
        let result = codec.decode(&mut buf).unwrap();
        // No complete item yet (line_buf has "hello", telnet_state is Iac)
        assert!(result.is_none());

        // Complete the NOP in the next buffer
        let mut buf = BytesMut::from(&b"\xF1\n"[..]);
        let item = codec.decode(&mut buf).unwrap().unwrap();
        let cmd = expect_telnet_cmd(item);
        assert_eq!(cmd.as_ref(), &[0xFF, 0xF1]);

        // Then the line
        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "hello");
    }

    #[test]
    fn test_telnet_incomplete_will_at_end() {
        let mut codec = ConnectionCodec::new();

        // IAC WILL at end of buffer — need one more byte
        let mut buf = BytesMut::from(&b"\xFF\xFB"[..]);
        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_none());

        // Complete with the option byte
        let mut buf = BytesMut::from(&b"\x01ok\n"[..]);
        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_telnet_cmd(item).as_ref(), &[0xFF, 0xFB, 0x01]);

        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "ok");
    }

    #[test]
    fn test_telnet_incomplete_subneg_at_end() {
        let mut codec = ConnectionCodec::new();

        // Incomplete subnegotiation — no IAC SE yet
        let mut buf = BytesMut::from(&b"\xFF\xFA\x1F\x00\x50"[..]);
        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_none());

        // Complete the subnegotiation
        let mut buf = BytesMut::from(&b"\x00\x18\xFF\xF0done\n"[..]);
        let item = codec.decode(&mut buf).unwrap().unwrap();
        let cmd = expect_telnet_cmd(item);
        assert_eq!(
            cmd.as_ref(),
            &[0xFF, 0xFA, 0x1F, 0x00, 0x50, 0x00, 0x18, 0xFF, 0xF0]
        );

        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "done");
    }

    #[test]
    fn test_telnet_preserves_utf8() {
        let mut codec = ConnectionCodec::new();

        // IAC NOP followed by UTF-8 CJK text — must not corrupt the multi-byte chars
        let mut buf = BytesMut::new();
        buf.extend_from_slice(b"\xFF\xF1");
        buf.extend_from_slice("读写汉字\n".as_bytes());

        let item = codec.decode(&mut buf).unwrap().unwrap();
        expect_telnet_cmd(item); // NOP

        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "读写汉字");
    }

    #[test]
    fn test_control_chars_filtered() {
        let mut codec = ConnectionCodec::new();

        // Control characters (except tab) should be filtered from text
        let mut buf = BytesMut::from(&b"he\x01ll\x7Fo\tworld\n"[..]);

        let item = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expect_line(item), "hello\tworld");
    }
}
