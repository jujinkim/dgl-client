use crate::{Error, Result};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Response {
        timestamp: u64,
        number: u64,
        flags: u32,
        lines: Vec<Vec<u8>>,
        failed: bool,
    },
    Output {
        pane: String,
        bytes: Vec<u8>,
    },
    Notification {
        name: String,
        arguments: String,
    },
}
pub struct Parser {
    line: Vec<u8>,
    frame: Option<(u64, u64, u32)>,
    body: Vec<Vec<u8>>,
    response_bytes: usize,
    max_line: usize,
    max_response: usize,
    poisoned: bool,
}
impl Default for Parser {
    fn default() -> Self {
        Self::new(1 << 20, 4 << 20)
    }
}
impl Parser {
    pub fn new(max_line: usize, max_response: usize) -> Self {
        Self {
            line: vec![],
            frame: None,
            body: vec![],
            response_bytes: 0,
            max_line,
            max_response,
            poisoned: false,
        }
    }
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Event>> {
        if self.poisoned {
            return Err(Error::Protocol);
        }
        let result = (|| {
            let mut out = vec![];
            for &b in bytes {
                if b == b'\n' {
                    let line = std::mem::take(&mut self.line);
                    self.parse_line(line, &mut out)?;
                } else {
                    if self.line.len() >= self.max_line {
                        return Err(Error::Protocol);
                    }
                    self.line.push(b);
                }
            }
            Ok(out)
        })();
        if result.is_err() {
            self.poisoned = true;
            self.line.clear();
            self.body.clear();
        }
        result
    }
    pub fn finish(&mut self) -> Result<()> {
        if self.poisoned || !self.line.is_empty() || self.frame.is_some() {
            self.poisoned = true;
            Err(Error::Protocol)
        } else {
            Ok(())
        }
    }
    fn parse_line(&mut self, raw: Vec<u8>, out: &mut Vec<Event>) -> Result<()> {
        let separator = raw.iter().position(|&b| b == b' ');
        let end = separator.unwrap_or(raw.len());
        let name = std::str::from_utf8(&raw[..end]).map_err(|_| Error::Protocol)?;
        if let Some((timestamp, number, flags)) = self.frame {
            if (name == "%end" || name == "%error") && boundary(&raw) == self.frame {
                out.push(Event::Response {
                    timestamp,
                    number,
                    flags,
                    lines: std::mem::take(&mut self.body),
                    failed: name == "%error",
                });
                self.frame = None;
                self.response_bytes = 0;
            } else {
                self.response_bytes = self
                    .response_bytes
                    .checked_add(raw.len() + 1)
                    .ok_or(Error::Protocol)?;
                if self.response_bytes > self.max_response {
                    return Err(Error::Protocol);
                }
                self.body.push(raw);
            }
            return Ok(());
        }
        match name {
            "%begin" => {
                self.frame = Some(boundary(&raw).ok_or(Error::Protocol)?);
            }
            "%end" | "%error" => return Err(Error::Protocol),
            "%output" | "%extended-output" => {
                let start = separator.ok_or(Error::Protocol)? + 1;
                let pane_end = raw[start..]
                    .iter()
                    .position(|&b| b == b' ')
                    .ok_or(Error::Protocol)?
                    + start;
                let pane =
                    std::str::from_utf8(&raw[start..pane_end]).map_err(|_| Error::Protocol)?;
                crate::command::target(pane, '%').map_err(|_| Error::Protocol)?;
                let mut body = pane_end + 1;
                if name == "%extended-output" {
                    body += raw[body..]
                        .windows(2)
                        .position(|w| w == b": ")
                        .ok_or(Error::Protocol)?
                        + 2;
                }
                out.push(Event::Output {
                    pane: pane.into(),
                    bytes: decode(&raw[body..], false)?,
                });
            }
            _ => {
                if !name.starts_with('%') {
                    return Err(Error::Protocol);
                }
                out.push(Event::Notification {
                    name: name.into(),
                    arguments: String::from_utf8_lossy(if end < raw.len() {
                        &raw[end + 1..]
                    } else {
                        &[]
                    })
                    .into_owned(),
                });
            }
        }
        Ok(())
    }
}
fn boundary(raw: &[u8]) -> Option<(u64, u64, u32)> {
    let s = std::str::from_utf8(raw).ok()?;
    let f: Vec<_> = s.split(' ').collect();
    if f.len() != 4 {
        return None;
    }
    let t = f[1].parse::<i64>().ok()?;
    let n = f[2].parse::<i64>().ok()?;
    let flags = f[3].parse::<i32>().ok()?;
    if t < 0 || n < 0 || flags < 0 {
        return None;
    }
    Some((t as u64, n as u64, flags as u32))
}
pub fn decode(bytes: &[u8], capture: bool) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            if capture && bytes.get(i + 1) == Some(&b'\\') {
                out.push(b'\\');
                i += 2;
                continue;
            }
            let digits = bytes.get(i + 1..i + 4).ok_or(Error::Protocol)?;
            let mut v = 0u16;
            for d in digits {
                if !(b'0'..=b'7').contains(d) {
                    return Err(Error::Protocol);
                }
                v = v * 8 + u16::from(d - b'0');
            }
            if v > 255 {
                return Err(Error::Protocol);
            }
            out.push(v as u8);
            i += 4;
        } else {
            out.push(bytes[i]);
            i += 1
        }
    }
    Ok(out)
}
