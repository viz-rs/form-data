use std::{
    fs::File,
    io::{Error as IoError, ErrorKind, Read, Write},
};

use bytes::{Bytes, BytesMut};
use http::{
    header::{CONTENT_DISPOSITION, CONTENT_TYPE},
    HeaderValue,
};
use tracing::trace;

use crate::{
    utils::{parse_content_disposition, parse_content_type, parse_part_headers},
    Error, Field, Flag, FormData, Result, State,
};

impl<T> Read for State<T>
where
    T: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        self.io_mut().read(buf)
    }
}

impl<T> Iterator for State<T>
where
    T: Read,
{
    type Item = Result<Bytes>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.is_readable {
                // part
                trace!("attempting to decode a part");

                // field
                if let Some(data) = self.decode() {
                    trace!("part decoded from buffer");
                    return Some(Ok(data));
                }

                // field stream is ended
                if Flag::Next == self.flag {
                    return None;
                }

                // whole stream is ended
                if Flag::Eof == self.flag {
                    self.length -= self.buffer.len() as u64;
                    self.buffer.clear();
                    self.eof = true;
                    return None;
                }

                self.is_readable = false;
            }

            trace!("polling data from stream");

            if self.eof {
                self.is_readable = true;
                continue;
            }

            self.buffer.reserve(1);
            let mut b = BytesMut::new();
            b.resize(self.limits.buffer_size, 0);
            let bytect = match self.read(&mut b) {
                Err(e) => return Some(Err(e.into())),
                Ok(s) => {
                    let l = s as u64;
                    if let Some(max) = self.limits.checked_stream_size(self.length + l) {
                        return Some(Err(Error::PayloadTooLarge(max)));
                    }

                    self.buffer.extend_from_slice(&b.split_to(s));
                    self.length += l;
                    l
                }
            };

            if bytect == 0 {
                self.eof = true;
            }

            self.is_readable = true;
        }
    }
}

impl<T> Read for Field<T>
where
    T: Read,
{
    fn read(&mut self, mut buf: &mut [u8]) -> Result<usize, IoError> {
        match self.next() {
            None => Ok(0),
            Some(Ok(b)) => buf.write(&b),
            Some(Err(e)) => Err(IoError::new(ErrorKind::Other, e)),
        }
    }
}

impl<T> Field<T>
where
    T: Read,
{
    /// Reads field data to bytes.
    pub fn bytes(&mut self) -> Result<Bytes> {
        let mut bytes = BytesMut::new();
        while let Some(buf) = self.next() {
            bytes.extend_from_slice(&buf?);
        }
        Ok(bytes.freeze())
    }

    /// Copys bytes to a writer.
    pub fn copy_to<W>(&mut self, writer: &mut W) -> Result<u64>
    where
        W: Write + Send + Unpin + 'static,
    {
        let mut n = 0;
        while let Some(buf) = self.next() {
            let b = buf?;
            writer.write_all(&b)?;
            n += b.len();
        }
        writer.flush()?;
        Ok(n as u64)
    }

    /// Copys bytes to a File.
    pub fn copy_to_file(&mut self, file: &mut File) -> Result<u64> {
        let mut n = 0;
        while let Some(buf) = self.next() {
            n += file.write(&buf?)?;
        }
        file.flush()?;
        Ok(n as u64)
    }

    /// Ignores current field data, pass it.
    pub fn ignore(&mut self) -> Result<()> {
        while let Some(buf) = self.next() {
            drop(buf?);
        }
        Ok(())
    }
}

impl<T> Iterator for Field<T>
where
    T: Read,
{
    type Item = Result<Bytes>;

    fn next(&mut self) -> Option<Self::Item> {
        trace!("polling {} {}", self.index, self.state.is_some());

        let state = self.state.clone()?;
        let mut state = state
            .try_lock()
            .map_err(|e| Error::TryLockError(e.to_string()))
            .ok()?;
        let is_file = self.filename.is_some();

        match state.next().and_then(Result::ok) {
            None => {
                trace!("polled {}", self.index);
                drop(self.state.take());
                None
            }
            Some(buf) => {
                let l = buf.len();

                if is_file {
                    if let Some(max) = state.limits.checked_file_size(self.length + l) {
                        return Some(Err(Error::FileTooLarge(max)));
                    }
                } else if let Some(max) = state.limits.checked_field_size(self.length + l) {
                    return Some(Err(Error::FieldTooLarge(max)));
                }

                self.length += l;
                trace!("polled bytes {}/{}", buf.len(), self.length);
                Some(Ok(buf))
            }
        }
    }
}

/// Reads form-data from request payload body, then yields `Field`
impl<T> Iterator for FormData<T>
where
    T: Read,
{
    type Item = Result<Field<T>>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut state = self
            .state
            .try_lock()
            .map_err(|e| Error::TryLockError(e.to_string()))
            .ok()?;

        match state.next()? {
            Err(e) => Some(Err(e)),
            Ok(buf) => {
                trace!("parse part");

                // too many parts
                if let Some(max) = state.limits.checked_parts(state.total + 1) {
                    return Some(Err(Error::PartsTooMany(max)));
                }

                // invalid part header
                let Ok(mut headers) = parse_part_headers(&buf) else {
                    return Some(Err(Error::InvalidHeader));
                };

                // invalid content disposition
                let Some((name, filename)) = headers
                    .remove(CONTENT_DISPOSITION)
                    .as_ref()
                    .map(HeaderValue::as_bytes)
                    .map(parse_content_disposition)
                    .and_then(Result::ok)
                else {
                    return Some(Err(Error::InvalidContentDisposition));
                };

                // field name is too long
                if let Some(max) = state.limits.checked_field_name_size(name.len()) {
                    return Some(Err(Error::FieldNameTooLong(max)));
                }

                if filename.is_some() {
                    // files too many
                    if let Some(max) = state.limits.checked_files(state.files + 1) {
                        return Some(Err(Error::FilesTooMany(max)));
                    }
                    state.files += 1;
                } else {
                    // fields too many
                    if let Some(max) = state.limits.checked_fields(state.fields + 1) {
                        return Some(Err(Error::FieldsTooMany(max)));
                    }
                    state.fields += 1;
                }

                // yields `Field`
                let mut field = Field::empty();

                field.name = name;
                field.filename = filename;
                field.index = state.index();
                field.content_type = parse_content_type(headers.remove(CONTENT_TYPE).as_ref());
                field.state_mut().replace(self.state());

                if !headers.is_empty() {
                    field.headers_mut().replace(headers);
                }

                Some(Ok(field))
            }
        }
    }
}
